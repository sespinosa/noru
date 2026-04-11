use anyhow::Result;

use crate::types::DetectedMeeting;

pub struct Detector;

impl Detector {
    pub fn new() -> Self {
        Self
    }

    pub fn poll(&self) -> Result<Option<DetectedMeeting>> {
        unimplemented!("detect::Detector::poll — Phase 2 backend teammate")
    }

    pub fn known_platforms(&self) -> &'static [&'static str] {
        &["zoom", "teams", "meet", "slack", "discord", "webex"]
    }
}

impl Default for Detector {
    fn default() -> Self {
        Self::new()
    }
}
