use anyhow::Result;

use crate::types::{AuthFlowHandle, AuthStatus};

/// Codex CLI public client id — the ChatGPT OAuth client we piggyback on.
/// See PLAN.md section 4.6 for rationale; this is explicitly labeled
/// experimental throughout the product because it is an unofficial flow.
pub const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";

/// Begin a new OAuth sign-in flow.
///
/// Generates a PKCE verifier + challenge, builds the authorize URL against
/// `https://auth.openai.com/oauth/authorize`, opens the URL in the user's
/// default browser, and spins up a loopback HTTP listener on a random
/// localhost port to catch the `/callback?code=...&state=...` redirect.
///
/// Returns a handle the frontend can poll (via `status`) until sign-in
/// completes or the user cancels. The flow_id is opaque to the frontend.
pub fn start_login() -> Result<AuthFlowHandle> {
    unimplemented!("auth::start_login — Phase 2 auth-ai teammate")
}

/// Complete an in-flight OAuth flow by verifying `state` matches what was
/// issued in `start_login`, exchanging `code` + PKCE verifier for tokens at
/// `https://auth.openai.com/oauth/token`, and persisting them to
/// `~/.noru/auth.json` (mode 0600 on Unix; Windows ACL-restricted).
pub fn complete(_handle: AuthFlowHandle, _code: &str, _state: &str) -> Result<AuthStatus> {
    unimplemented!("auth::complete — Phase 2 auth-ai teammate")
}

/// Current signed-in status. Reads `~/.noru/auth.json` if present.
pub fn status() -> Result<AuthStatus> {
    unimplemented!("auth::status — Phase 2 auth-ai teammate")
}

/// Clear the stored token by deleting `~/.noru/auth.json`.
pub fn sign_out() -> Result<()> {
    unimplemented!("auth::sign_out — Phase 2 auth-ai teammate")
}

/// Return a valid access token, refreshing against `auth.openai.com/oauth/token`
/// if the stored one has expired. Errors if no token is stored.
pub fn access_token() -> Result<String> {
    unimplemented!("auth::access_token — Phase 2 auth-ai teammate")
}
