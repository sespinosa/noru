use anyhow::Result;

use crate::types::{MeetingState, MeetingStateChange, Platform};

/// Handle returned by `start` — dropping it (or calling `stop`) halts the
/// background polling loop.
pub struct DetectHandle {
    #[allow(dead_code)]
    pub(crate) _private: (),
}

impl DetectHandle {
    pub fn stop(self) {
        unimplemented!("detect::DetectHandle::stop — Phase 2 backend teammate")
    }
}

/// One-shot detection poll. Reads the current process / window snapshot and
/// returns a `MeetingState` without debouncing.
pub fn poll() -> Result<MeetingState> {
    unimplemented!("detect::poll — Phase 2 backend teammate")
}

/// List of platforms this module can detect.
pub fn known_platforms() -> &'static [Platform] {
    &[
        Platform::Zoom,
        Platform::Meet,
        Platform::Teams,
        Platform::Slack,
        Platform::Discord,
        Platform::Webex,
    ]
}

/// Start a background polling loop. The callback fires only on state
/// transitions (meeting started / ended), after the ≥3-poll debounce.
/// The polling interval is implementation-defined (~5s in v1).
pub fn start<F>(_callback: F) -> Result<DetectHandle>
where
    F: Fn(MeetingStateChange) + Send + 'static,
{
    unimplemented!("detect::start — Phase 2 backend teammate")
}

/// Pure-function test hook for the window-title matching logic. Given a raw
/// window title and its owning process name, returns the detected platform if
/// the title matches a "meeting active" pattern for that process.
pub fn parse_window_title_for_meeting(_title: &str, _process_name: &str) -> Option<Platform> {
    unimplemented!("detect::parse_window_title_for_meeting — Phase 2 backend teammate")
}
