use anyhow::Result;

use crate::types::{Meeting, MeetingId, MeetingSummary, NewMeeting};

/// Open the sqlite connection at `~/.noru/noru.db`, run any pending migrations,
/// and make the global handle ready for subsequent calls. Must be called once
/// at app boot, before any other storage function.
pub fn init() -> Result<()> {
    unimplemented!("storage::init — Phase 2 backend teammate")
}

/// Insert a new meeting row and return its freshly minted MeetingId (ULID).
pub fn save_meeting(_meeting: NewMeeting) -> Result<MeetingId> {
    unimplemented!("storage::save_meeting — Phase 2 backend teammate")
}

/// Fetch a single meeting by id. Returns `None` if no row matches.
pub fn get_meeting(_id: &MeetingId) -> Result<Option<Meeting>> {
    unimplemented!("storage::get_meeting — Phase 2 backend teammate")
}

/// Paginated list of meetings, newest first.
pub fn list_meetings(_limit: usize, _offset: usize) -> Result<Vec<MeetingSummary>> {
    unimplemented!("storage::list_meetings — Phase 2 backend teammate")
}

/// Persist an AI-generated summary for the given meeting.
pub fn update_summary(_id: &MeetingId, _summary: &str) -> Result<()> {
    unimplemented!("storage::update_summary — Phase 2 backend teammate")
}

/// Persist extracted action items (JSON-serialized list) for the given meeting.
pub fn update_action_items(_id: &MeetingId, _items: &[String]) -> Result<()> {
    unimplemented!("storage::update_action_items — Phase 2 backend teammate")
}

/// Persist extracted key decisions (JSON-serialized list) for the given meeting.
pub fn update_key_decisions(_id: &MeetingId, _decisions: &[String]) -> Result<()> {
    unimplemented!("storage::update_key_decisions — Phase 2 backend teammate")
}

/// Delete a meeting row and all its associated data.
pub fn delete_meeting(_id: &MeetingId) -> Result<()> {
    unimplemented!("storage::delete_meeting — Phase 2 backend teammate")
}
