use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

use crate::auth;

/// Codex backend base URL — the ChatGPT-authenticated entrypoint used by the
/// upstream Codex CLI (see `EvanZhouDev/openai-oauth` transport.ts,
/// `DEFAULT_CODEX_BASE_URL`). The `/responses` subpath is the OpenAI Responses
/// API shape.
const CODEX_RESPONSES_URL: &str = "https://chatgpt.com/backend-api/codex/responses";

/// Model identifier sent to the Codex backend. The Codex CLI rotates this as
/// OpenAI ships new defaults; `gpt-5` is the one upstream currently lands on
/// as of early 2026.
const DEFAULT_MODEL: &str = "gpt-5";

/// Concise paragraph summary of a meeting transcript.
pub fn summarize(transcript: &str) -> Result<String> {
    let instructions = "You are a meeting assistant. Summarize the following meeting \
        transcript in a single concise paragraph of 2-4 sentences. Focus on the main \
        topic, decisions reached, and notable outcomes. Do not add preamble or \
        closing remarks — respond with the summary text only.";
    let text = call_codex(instructions, transcript)?;
    Ok(text.trim().to_string())
}

/// Bulleted list of action items mentioned in the meeting.
pub fn extract_action_items(transcript: &str) -> Result<Vec<String>> {
    let instructions = "You are a meeting assistant. Extract the action items from the \
        following meeting transcript. An action item is a concrete task someone \
        committed to doing. Respond ONLY with a JSON object of the exact form \
        {\"items\": [\"first action\", \"second action\"]}. If there are no action \
        items, respond with {\"items\": []}. Do not include any text outside the JSON.";
    let text = call_codex(instructions, transcript)?;
    parse_items(&text)
}

/// Bulleted list of decisions made in the meeting.
pub fn extract_key_decisions(transcript: &str) -> Result<Vec<String>> {
    let instructions = "You are a meeting assistant. Extract the key decisions made in \
        the following meeting transcript. A key decision is a choice the group \
        explicitly agreed on. Respond ONLY with a JSON object of the exact form \
        {\"items\": [\"first decision\", \"second decision\"]}. If no decisions were \
        made, respond with {\"items\": []}. Do not include any text outside the JSON.";
    let text = call_codex(instructions, transcript)?;
    parse_items(&text)
}

// ---------------------------------------------------------------------------
// Transport — single POST to the Codex Responses endpoint, non-streaming.
// ---------------------------------------------------------------------------

fn call_codex(instructions: &str, transcript: &str) -> Result<String> {
    let auth = auth::effective_auth()?;

    let body = serde_json::json!({
        "model": DEFAULT_MODEL,
        "instructions": instructions,
        "input": transcript,
        "stream": false,
        "store": false,
    });

    let resp = ureq::post(CODEX_RESPONSES_URL)
        .set("Authorization", &format!("Bearer {}", auth.access_token))
        .set("chatgpt-account-id", &auth.account_id)
        .set("OpenAI-Beta", "responses=experimental")
        .set("Content-Type", "application/json")
        .set("Originator", "noru")
        .send_json(body);

    let raw = match resp {
        Ok(r) => r.into_string().context("reading Codex response body")?,
        Err(ureq::Error::Status(code, r)) => {
            let msg = r.into_string().unwrap_or_default();
            return Err(anyhow!(
                "Codex backend returned {code}: {msg}. If this says the OAuth flow is \
                 invalid, sign out from Settings → AI Features (experimental) and sign \
                 in again."
            ));
        }
        Err(e) => return Err(anyhow!("Codex request failed: {e}")),
    };

    extract_output_text(&raw)
        .ok_or_else(|| anyhow!("could not locate output text in Codex response: {raw}"))
}

/// Walk a Codex Responses-API reply and pull the assistant message text.
///
/// The shape we target:
/// ```json
/// {
///   "output_text": "...",
///   "output": [{"type":"message","content":[{"type":"output_text","text":"..."}]}]
/// }
/// ```
/// We prefer the top-level `output_text` convenience field when present, then
/// fall back to walking `output[].content[]` for `output_text` nodes.
fn extract_output_text(raw: &str) -> Option<String> {
    #[derive(Deserialize)]
    struct Envelope {
        #[serde(default)]
        output_text: Option<String>,
        #[serde(default)]
        output: Vec<OutputItem>,
    }
    #[derive(Deserialize)]
    struct OutputItem {
        #[serde(default)]
        content: Vec<ContentPart>,
    }
    #[derive(Deserialize)]
    struct ContentPart {
        #[serde(rename = "type", default)]
        kind: String,
        #[serde(default)]
        text: Option<String>,
    }

    let parsed: Envelope = serde_json::from_str(raw).ok()?;
    if let Some(t) = parsed.output_text {
        if !t.is_empty() {
            return Some(t);
        }
    }
    let mut collected = String::new();
    for item in parsed.output {
        for part in item.content {
            if part.kind == "output_text" {
                if let Some(t) = part.text {
                    if !collected.is_empty() {
                        collected.push('\n');
                    }
                    collected.push_str(&t);
                }
            }
        }
    }
    if collected.is_empty() {
        None
    } else {
        Some(collected)
    }
}

fn parse_items(text: &str) -> Result<Vec<String>> {
    #[derive(Deserialize)]
    struct Items {
        items: Vec<String>,
    }
    // Models occasionally wrap JSON in ``` fences despite instructions.
    let cleaned = strip_code_fence(text.trim());
    let parsed: Items = serde_json::from_str(cleaned).with_context(|| {
        format!("parsing JSON items from model output: {cleaned}")
    })?;
    Ok(parsed
        .items
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

fn strip_code_fence(s: &str) -> &str {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("```json") {
        return rest.trim().strip_suffix("```").unwrap_or(rest).trim();
    }
    if let Some(rest) = s.strip_prefix("```") {
        return rest.trim().strip_suffix("```").unwrap_or(rest).trim();
    }
    s
}
