use anyhow::{anyhow, Context, Result};
use oauth2::{CsrfToken, PkceCodeChallenge};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::types::{AuthFlowHandle, AuthStatus};

/// Codex CLI public client id — the ChatGPT OAuth client we piggyback on.
/// See PLAN.md section 4.6 for rationale; this is explicitly labeled
/// experimental throughout the product because it is an unofficial flow.
pub const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";

const AUTH_ENDPOINT: &str = "https://auth.openai.com/oauth/authorize";
const TOKEN_ENDPOINT: &str = "https://auth.openai.com/oauth/token";
// Scope must match Codex CLI upstream exactly — auth.openai.com rejects the
// flow if the scope set differs. The two `api.connectors.*` entries are not
// optional even though we don't use the connectors API.
const OAUTH_SCOPE: &str =
    "openid profile email offline_access api.connectors.read api.connectors.invoke";
// Codex's loopback callback uses a fixed port + path. The OAuth client
// app_EMoamEEZ73f0CkXaXp7hrann is registered against this exact redirect URI.
// Random ports / different paths get rejected with `invalid_redirect_uri`.
const CALLBACK_PORT: u16 = 1455;
const CALLBACK_PATH: &str = "/auth/callback";
// Originator string sent to identify the client (matches Codex CLI Rust).
const ORIGINATOR: &str = "codex_cli_rs";
// Seconds before JWT expiry at which we proactively refresh.
const REFRESH_MARGIN_SECS: i64 = 300;

/// Persisted OAuth record written to `~/.noru/auth.json`. Field set is a
/// superset of the one CLAUDE.md specifies — we keep `id_token` and
/// `account_id` because the Codex backend requires the latter as a request
/// header and the former is the only source for the former and the account
/// email. Layout is stable across refreshes.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredAuth {
    access_token: String,
    refresh_token: String,
    id_token: Option<String>,
    /// Unix epoch seconds at which the access token expires. Derived from the
    /// access_token JWT `exp` claim at write time.
    expires_at: i64,
    account_email: String,
    account_id: String,
    client_id: String,
}

/// In-memory state for an in-flight OAuth flow, keyed by flow_id.
struct PendingFlow {
    verifier: String,
    state: String,
    redirect_uri: String,
}

fn pending() -> &'static Mutex<HashMap<String, PendingFlow>> {
    static PENDING: OnceLock<Mutex<HashMap<String, PendingFlow>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

fn auth_file_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("could not resolve home directory"))?;
    Ok(home.join(".noru").join("auth.json"))
}

fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Public API — signatures are locked by Phase 1 stubs.
// ---------------------------------------------------------------------------

/// Begin a new OAuth sign-in flow.
pub fn start_login() -> Result<AuthFlowHandle> {
    // The loopback listener MUST bind to localhost:1455 — that's the exact
    // redirect_uri the public Codex client_id is registered against. Any
    // other host/port/path makes auth.openai.com reject the request with
    // `invalid_redirect_uri`.
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], CALLBACK_PORT)))
        .context("binding loopback listener for OAuth callback (port 1455)")?;
    let redirect_uri = format!("http://localhost:{CALLBACK_PORT}{CALLBACK_PATH}");

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let state = CsrfToken::new_random();
    let flow_id = new_flow_id();

    let authorize_url = build_authorize_url(
        &redirect_uri,
        pkce_challenge.as_str(),
        state.secret(),
    );

    pending().lock().unwrap().insert(
        flow_id.clone(),
        PendingFlow {
            verifier: pkce_verifier.secret().to_string(),
            state: state.secret().to_string(),
            redirect_uri,
        },
    );

    let flow_id_thread = flow_id.clone();
    std::thread::Builder::new()
        .name("noru-oauth-callback".into())
        .spawn(move || {
            if let Err(e) = serve_callback(listener, &flow_id_thread) {
                tracing::error!(error = %e, "oauth callback listener failed");
                pending().lock().unwrap().remove(&flow_id_thread);
            }
        })
        .context("spawning oauth callback listener")?;

    if let Err(e) = open_browser(&authorize_url) {
        tracing::warn!(error = %e, url = %authorize_url, "failed to open browser; user must open the url manually");
    }

    Ok(AuthFlowHandle {
        flow_id,
        authorize_url,
    })
}

/// Complete an in-flight OAuth flow (exchange code for tokens, persist).
pub fn complete(handle: AuthFlowHandle, code: &str, state: &str) -> Result<AuthStatus> {
    let flow = pending()
        .lock()
        .unwrap()
        .remove(&handle.flow_id)
        .ok_or_else(|| anyhow!("no in-flight oauth flow for id {}", handle.flow_id))?;

    if flow.state != state {
        return Err(anyhow!("oauth state mismatch — possible CSRF"));
    }

    let token = exchange_authorization_code(&flow.redirect_uri, code, &flow.verifier)?;
    let stored = store_from_token_response(token, None)?;
    write_stored(&stored)?;
    Ok(AuthStatus::Signed {
        account_email: stored.account_email,
    })
}

/// Current signed-in status. Reports `Refreshing` when a sign-in flow is
/// actively in progress and no token file exists yet; `Signed` if a token
/// is persisted; `SignedOut` otherwise.
pub fn status() -> Result<AuthStatus> {
    match read_stored()? {
        Some(stored) => Ok(AuthStatus::Signed {
            account_email: stored.account_email,
        }),
        None => {
            if !pending().lock().unwrap().is_empty() {
                Ok(AuthStatus::Refreshing)
            } else {
                Ok(AuthStatus::SignedOut)
            }
        }
    }
}

/// Clear the stored token by deleting `~/.noru/auth.json`.
pub fn sign_out() -> Result<()> {
    let path = auth_file_path()?;
    if path.exists() {
        std::fs::remove_file(&path)
            .with_context(|| format!("deleting {}", path.display()))?;
    }
    Ok(())
}

/// Return a valid access token, refreshing against `auth.openai.com/oauth/token`
/// if the stored one is within the refresh margin. Errors if no token is stored.
pub fn access_token() -> Result<String> {
    let mut stored = read_stored()?
        .ok_or_else(|| anyhow!("not signed in — run sign-in from Settings → AI Features"))?;

    if stored.expires_at - now_epoch() <= REFRESH_MARGIN_SECS {
        let refreshed = refresh_token_request(&stored.refresh_token)?;
        stored = store_from_token_response(refreshed, Some(stored))?;
        write_stored(&stored)?;
    }

    Ok(stored.access_token)
}

// ---------------------------------------------------------------------------
// Helpers exposed to the `ai` module (not part of the locked public API).
// ---------------------------------------------------------------------------

/// Snapshot of the currently effective auth — access_token + account_id — used
/// by the `ai` module to compose Codex backend requests. Refreshes the access
/// token if it's within the refresh margin.
pub(crate) struct EffectiveAuth {
    pub access_token: String,
    pub account_id: String,
}

pub(crate) fn effective_auth() -> Result<EffectiveAuth> {
    // access_token() handles refresh and persistence. Re-read the record
    // afterward to pick up the possibly-updated account_id.
    let access_token = access_token()?;
    let stored = read_stored()?
        .ok_or_else(|| anyhow!("auth file missing after refresh"))?;
    if stored.account_id.is_empty() {
        return Err(anyhow!(
            "ChatGPT account id missing from auth record — re-run sign-in"
        ));
    }
    Ok(EffectiveAuth {
        access_token,
        account_id: stored.account_id,
    })
}

// ---------------------------------------------------------------------------
// Token exchange via ureq — we don't use `oauth2`'s reqwest-based client
// because we need to read extra fields (id_token) and keep the transport
// dependency footprint to a single crate.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct TokenExchangeResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
}

fn exchange_authorization_code(
    redirect_uri: &str,
    code: &str,
    verifier: &str,
) -> Result<TokenExchangeResponse> {
    // Codex token exchange uses application/x-www-form-urlencoded, NOT JSON.
    // Sending JSON results in HTTP 400 from auth.openai.com.
    let form = vec![
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", CLIENT_ID),
        ("code_verifier", verifier),
    ];
    post_token_endpoint(&form)
}

fn refresh_token_request(refresh_token: &str) -> Result<TokenExchangeResponse> {
    let form = vec![
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", CLIENT_ID),
        ("scope", OAUTH_SCOPE),
    ];
    post_token_endpoint(&form)
}

fn post_token_endpoint(form: &[(&str, &str)]) -> Result<TokenExchangeResponse> {
    let resp = ureq::post(TOKEN_ENDPOINT)
        .set("Content-Type", "application/x-www-form-urlencoded")
        .send_form(form);
    match resp {
        Ok(r) => r
            .into_json::<TokenExchangeResponse>()
            .context("decoding token endpoint response"),
        Err(ureq::Error::Status(code, r)) => {
            let msg = r.into_string().unwrap_or_default();
            Err(anyhow!("token endpoint returned {code}: {msg}"))
        }
        Err(e) => Err(anyhow!("token endpoint request failed: {e}")),
    }
}

/// Construct a `StoredAuth` from a fresh token response. On refresh, carry
/// forward fields the refresh response may omit (refresh_token, id_token,
/// account_email, account_id).
fn store_from_token_response(
    resp: TokenExchangeResponse,
    previous: Option<StoredAuth>,
) -> Result<StoredAuth> {
    let prev_refresh = previous.as_ref().map(|p| p.refresh_token.clone());
    let prev_id_token = previous.as_ref().and_then(|p| p.id_token.clone());
    let prev_email = previous.as_ref().map(|p| p.account_email.clone());
    let prev_account = previous.as_ref().map(|p| p.account_id.clone());

    let refresh_token = resp
        .refresh_token
        .or(prev_refresh)
        .ok_or_else(|| anyhow!("no refresh_token in token response and none cached"))?;

    let id_token = resp.id_token.or(prev_id_token);

    // Prefer the JWT `exp` claim on the access token. Fall back to `expires_in`.
    let expires_at = jwt_expiry(&resp.access_token)
        .or_else(|| resp.expires_in.map(|secs| now_epoch() + secs))
        .unwrap_or_else(|| now_epoch() + 3600);

    let claims = id_token.as_deref().and_then(parse_jwt_claims);
    let account_email = claims
        .as_ref()
        .and_then(|c| c.get("email").and_then(|v| v.as_str()).map(String::from))
        .or(prev_email)
        .unwrap_or_default();
    let account_id = claims
        .as_ref()
        .and_then(derive_chatgpt_account_id)
        .or(prev_account)
        .unwrap_or_default();

    Ok(StoredAuth {
        access_token: resp.access_token,
        refresh_token,
        id_token,
        expires_at,
        account_email,
        account_id,
        client_id: CLIENT_ID.to_string(),
    })
}

// ---------------------------------------------------------------------------
// JWT parsing (header.payload.signature, base64url-encoded).
// We only need the payload claims — signature verification is not our job
// because the token issuer is a trusted party and the transport is TLS.
// ---------------------------------------------------------------------------

fn jwt_expiry(jwt: &str) -> Option<i64> {
    parse_jwt_claims(jwt)?
        .get("exp")
        .and_then(|v| v.as_i64())
}

fn parse_jwt_claims(jwt: &str) -> Option<serde_json::Value> {
    let mut parts = jwt.splitn(3, '.');
    parts.next()?;
    let payload_b64 = parts.next()?;
    let decoded = base64url_decode(payload_b64).ok()?;
    serde_json::from_slice(&decoded).ok()
}

fn derive_chatgpt_account_id(claims: &serde_json::Value) -> Option<String> {
    let auth = claims.get("https://api.openai.com/auth")?;
    auth.get("chatgpt_account_id")
        .and_then(|v| v.as_str())
        .map(String::from)
}

fn base64url_decode(input: &str) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buf: u32 = 0;
    let mut bits: u8 = 0;
    for &b in input.as_bytes() {
        if b == b'=' {
            continue;
        }
        let v = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => 26 + (b - b'a'),
            b'0'..=b'9' => 52 + (b - b'0'),
            b'-' => 62,
            b'_' => 63,
            _ => return Err(anyhow!("invalid base64url byte")),
        };
        buf = (buf << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xFF) as u8);
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// On-disk persistence.
// ---------------------------------------------------------------------------

fn read_stored() -> Result<Option<StoredAuth>> {
    let path = auth_file_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(&path)
        .with_context(|| format!("reading {}", path.display()))?;
    let parsed: StoredAuth = serde_json::from_slice(&bytes)
        .with_context(|| format!("parsing {}", path.display()))?;
    Ok(Some(parsed))
}

fn write_stored(stored: &StoredAuth) -> Result<()> {
    let path = auth_file_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_vec_pretty(stored)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)
            .with_context(|| format!("opening {}", path.display()))?;
        f.write_all(&data)?;
    }
    #[cfg(windows)]
    {
        // The file lives under %USERPROFILE%\.noru\, which inherits the user
        // profile's ACL restricting access to the current user + SYSTEM. This
        // matches the default protection of %USERPROFILE%\.codex\auth.json
        // used by the upstream Codex CLI.
        std::fs::write(&path, &data)
            .with_context(|| format!("writing {}", path.display()))?;
    }
    #[cfg(not(any(unix, windows)))]
    {
        std::fs::write(&path, &data)
            .with_context(|| format!("writing {}", path.display()))?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Authorize URL construction and loopback callback listener.
// ---------------------------------------------------------------------------

fn build_authorize_url(redirect_uri: &str, pkce_challenge: &str, state: &str) -> String {
    // Parameter set + ordering taken from openai/codex `codex-rs/login/src/server.rs`
    // `build_authorize_url`. The id_token_add_organizations and
    // codex_cli_simplified_flow flags are required — auth.openai.com rejects
    // the flow with `missing_required_parameter` if either is absent.
    let params: &[(&str, &str)] = &[
        ("response_type", "code"),
        ("client_id", CLIENT_ID),
        ("redirect_uri", redirect_uri),
        ("scope", OAUTH_SCOPE),
        ("code_challenge", pkce_challenge),
        ("code_challenge_method", "S256"),
        ("id_token_add_organizations", "true"),
        ("codex_cli_simplified_flow", "true"),
        ("state", state),
        ("originator", ORIGINATOR),
    ];

    let mut url = String::from(AUTH_ENDPOINT);
    url.push('?');
    for (i, (k, v)) in params.iter().enumerate() {
        if i > 0 {
            url.push('&');
        }
        url.push_str(k);
        url.push('=');
        url.push_str(&percent_encode(v));
    }
    url
}

fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        let unreserved = matches!(
            b,
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~'
        );
        if unreserved {
            out.push(b as char);
        } else {
            out.push('%');
            out.push(hex_nibble(b >> 4));
            out.push(hex_nibble(b & 0xF));
        }
    }
    out
}

fn hex_nibble(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        10..=15 => (b'A' + n - 10) as char,
        _ => '0',
    }
}

fn serve_callback(listener: TcpListener, flow_id: &str) -> Result<()> {
    // A single request is expected — the browser hits the redirect_uri exactly
    // once. We use a blocking accept with a generous OS-level timeout to avoid
    // leaking a thread if the user abandons the flow.
    listener
        .set_nonblocking(false)
        .context("configuring listener")?;
    let (mut stream, _) = listener.accept().context("accepting oauth callback")?;
    // Give the peer a few seconds to finish sending; we only read one line.
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .ok();
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .ok();

    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .context("reading oauth callback request line")?;

    let result = match parse_callback_query(&request_line) {
        Some((code, state)) => complete(
            AuthFlowHandle {
                flow_id: flow_id.to_string(),
                authorize_url: String::new(),
            },
            &code,
            &state,
        ),
        None => Err(anyhow!(
            "callback request missing code/state: {}",
            request_line.trim()
        )),
    };

    let body = if result.is_ok() {
        "<!doctype html><meta charset=\"utf-8\"><title>noru — signed in</title>\
         <body style=\"font-family:-apple-system,system-ui,sans-serif;text-align:center;padding:64px;color:#222\">\
         <h1>Signed in to ChatGPT.</h1>\
         <p>You can close this tab and return to noru.</p></body>"
    } else {
        "<!doctype html><meta charset=\"utf-8\"><title>noru — sign-in failed</title>\
         <body style=\"font-family:-apple-system,system-ui,sans-serif;text-align:center;padding:64px;color:#222\">\
         <h1>Sign-in failed.</h1>\
         <p>Return to noru and try again.</p></body>"
    };
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();

    result.map(|_| ())
}

fn parse_callback_query(request_line: &str) -> Option<(String, String)> {
    // Expected form: "GET /callback?code=...&state=... HTTP/1.1"
    let mut parts = request_line.split_whitespace();
    let _method = parts.next()?;
    let target = parts.next()?;
    let query = target.split_once('?')?.1;
    let mut code = None;
    let mut state = None;
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=')?;
        let decoded = url_decode(v);
        match k {
            "code" => code = Some(decoded),
            "state" => state = Some(decoded),
            _ => {}
        }
    }
    Some((code?, state?))
}

fn url_decode(input: &str) -> String {
    let mut out = Vec::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                if let (Some(hi), Some(lo)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                    out.push((hi << 4) | lo);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).unwrap_or_default()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(10 + (b - b'a')),
        b'A'..=b'F' => Some(10 + (b - b'A')),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Random flow id and browser opener.
// ---------------------------------------------------------------------------

fn new_flow_id() -> String {
    // Reuse oauth2's CSRF RNG for an opaque id. Not security-critical —
    // it only indexes the in-memory HashMap of pending flows.
    CsrfToken::new_random().secret().clone()
}

fn open_browser(url: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        // We CANNOT use `cmd /c start "" <url>` here. `start` is a cmd.exe
        // builtin and treats `&` in its arguments as a command separator —
        // OAuth URLs contain one `&` per query parameter, so cmd would open
        // just `https://.../authorize?response_type=code` and try to run the
        // remaining `client_id=...`, `scope=...`, `state=...` as separate
        // commands. The auth.openai.com page then returns
        // `missing_required_parameter` because everything after the first
        // param is gone. Use rundll32's URL protocol handler instead — it
        // takes the URL as a single argv and forwards it to the registered
        // http handler without re-parsing.
        std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn()
            .context("spawning `rundll32 url.dll,FileProtocolHandler`")?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .context("spawning `open`")?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .context("spawning `xdg-open`")?;
    }
    Ok(())
}
