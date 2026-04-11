use anyhow::Result;

use crate::types::{Transcript, TranscriptSegment, TranscriptSummary};

pub struct Storage;

impl Storage {
    pub fn open() -> Result<Self> {
        unimplemented!("storage::Storage::open — Phase 2 backend teammate")
    }

    pub fn list_transcripts(&self) -> Result<Vec<TranscriptSummary>> {
        unimplemented!("storage::list_transcripts — Phase 2 backend teammate")
    }

    pub fn get_transcript(&self, _id: i64) -> Result<Transcript> {
        unimplemented!("storage::get_transcript — Phase 2 backend teammate")
    }

    pub fn create_transcript(
        &self,
        _started_at: i64,
        _platform: Option<&str>,
        _audio_path: Option<&str>,
    ) -> Result<i64> {
        unimplemented!("storage::create_transcript — Phase 2 backend teammate")
    }

    pub fn append_segments(&self, _id: i64, _segments: &[TranscriptSegment]) -> Result<()> {
        unimplemented!("storage::append_segments — Phase 2 backend teammate")
    }

    pub fn finalize_transcript(&self, _id: i64, _ended_at: i64) -> Result<()> {
        unimplemented!("storage::finalize_transcript — Phase 2 backend teammate")
    }

    pub fn save_ai_result(&self, _id: i64, _kind: AiResultKind, _value: &str) -> Result<()> {
        unimplemented!("storage::save_ai_result — Phase 2 backend teammate")
    }

    pub fn delete_transcript(&self, _id: i64) -> Result<()> {
        unimplemented!("storage::delete_transcript — Phase 2 backend teammate")
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AiResultKind {
    Summary,
    ActionItems,
    KeyDecisions,
}
