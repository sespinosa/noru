CREATE TABLE meetings (
  id TEXT PRIMARY KEY,
  started_at TEXT NOT NULL,
  ended_at TEXT,
  platform TEXT,
  audio_path TEXT,
  transcript_json TEXT,
  summary TEXT,
  action_items TEXT,
  key_decisions TEXT,
  created_at TEXT NOT NULL
);

CREATE INDEX idx_meetings_started_at ON meetings(started_at DESC);
