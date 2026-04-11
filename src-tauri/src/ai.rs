use anyhow::Result;

/// Concise paragraph summary of a meeting transcript.
///
/// Calls `auth::access_token()` internally, posts the transcript + a focused
/// system prompt to the Codex chat completion endpoint, and returns the
/// plain-text summary.
pub fn summarize(_transcript: &str) -> Result<String> {
    unimplemented!("ai::summarize — Phase 2 auth-ai teammate")
}

/// Bulleted list of action items mentioned in the meeting.
///
/// Uses JSON-mode output against the Codex chat completion endpoint.
pub fn extract_action_items(_transcript: &str) -> Result<Vec<String>> {
    unimplemented!("ai::extract_action_items — Phase 2 auth-ai teammate")
}

/// Bulleted list of decisions made in the meeting.
///
/// Uses JSON-mode output against the Codex chat completion endpoint.
pub fn extract_key_decisions(_transcript: &str) -> Result<Vec<String>> {
    unimplemented!("ai::extract_key_decisions — Phase 2 auth-ai teammate")
}
