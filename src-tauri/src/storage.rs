use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use anyhow::{anyhow, Result};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};

use crate::types::{Meeting, MeetingId, MeetingSummary, NewMeeting, Platform, TranscriptSegment};

static CONN: OnceLock<Mutex<Connection>> = OnceLock::new();

const MIGRATION_001: &str = include_str!("../migrations/001_initial.sql");

fn default_db_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("cannot determine home directory"))?;
    Ok(home.join(".noru").join("noru.db"))
}

fn open_connection(path: &PathBuf) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )?;
    conn.pragma_update(None, "journal_mode", &"WAL")?;
    conn.pragma_update(None, "foreign_keys", &"ON")?;
    run_migrations(&conn)?;
    Ok(conn)
}

fn run_migrations(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        )",
    )?;
    let current: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_version",
        [],
        |r| r.get(0),
    )?;
    if current < 1 {
        conn.execute_batch(MIGRATION_001)?;
        conn.execute(
            "INSERT INTO schema_version (version) VALUES (?1)",
            params![1],
        )?;
    }
    Ok(())
}

pub fn init() -> Result<()> {
    if CONN.get().is_some() {
        return Ok(());
    }
    let conn = open_connection(&default_db_path()?)?;
    let _ = CONN.set(Mutex::new(conn));
    Ok(())
}

fn with_conn<T>(f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
    let cell = CONN
        .get()
        .ok_or_else(|| anyhow!("storage not initialized; call storage::init first"))?;
    let conn = cell.lock().map_err(|_| anyhow!("storage mutex poisoned"))?;
    f(&conn)
}

pub fn save_meeting(meeting: NewMeeting) -> Result<MeetingId> {
    with_conn(|conn| save_meeting_on(conn, meeting))
}

pub fn get_meeting(id: &MeetingId) -> Result<Option<Meeting>> {
    with_conn(|conn| get_meeting_on(conn, id))
}

pub fn list_meetings(limit: usize, offset: usize) -> Result<Vec<MeetingSummary>> {
    with_conn(|conn| list_meetings_on(conn, limit, offset))
}

pub fn update_summary(id: &MeetingId, summary: &str) -> Result<()> {
    with_conn(|conn| update_summary_on(conn, id, summary))
}

pub fn update_action_items(id: &MeetingId, items: &[String]) -> Result<()> {
    with_conn(|conn| update_action_items_on(conn, id, items))
}

pub fn update_key_decisions(id: &MeetingId, decisions: &[String]) -> Result<()> {
    with_conn(|conn| update_key_decisions_on(conn, id, decisions))
}

pub fn delete_meeting(id: &MeetingId) -> Result<()> {
    with_conn(|conn| delete_meeting_on(conn, id))
}

fn save_meeting_on(conn: &Connection, meeting: NewMeeting) -> Result<MeetingId> {
    let id: String = conn.query_row("SELECT lower(hex(randomblob(16)))", [], |r| r.get(0))?;
    let transcript_json = serde_json::to_string(&meeting.segments)?;
    let platform_str = meeting.platform.map(platform_to_str);
    conn.execute(
        "INSERT INTO meetings (
            id, started_at, ended_at, platform, audio_path, transcript_json, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
        params![
            id,
            meeting.started_at,
            meeting.ended_at,
            platform_str,
            meeting.audio_path,
            transcript_json,
        ],
    )?;
    Ok(id)
}

fn get_meeting_on(conn: &Connection, id: &MeetingId) -> Result<Option<Meeting>> {
    conn.query_row(
        "SELECT id, started_at, ended_at, platform, audio_path, transcript_json,
                summary, action_items, key_decisions, created_at
         FROM meetings WHERE id = ?1",
        params![id],
        row_to_meeting,
    )
    .optional()
    .map_err(Into::into)
}

fn list_meetings_on(
    conn: &Connection,
    limit: usize,
    offset: usize,
) -> Result<Vec<MeetingSummary>> {
    let mut stmt = conn.prepare(
        "SELECT id, started_at, ended_at, platform, transcript_json, summary,
            CASE WHEN ended_at IS NULL THEN NULL
                 ELSE CAST((julianday(ended_at) - julianday(started_at)) * 86400000.0 AS INTEGER)
            END AS duration_ms
         FROM meetings
         ORDER BY started_at DESC
         LIMIT ?1 OFFSET ?2",
    )?;
    let rows = stmt.query_map(params![limit as i64, offset as i64], |row| {
        Ok(RowTuple {
            id: row.get(0)?,
            started_at: row.get(1)?,
            ended_at: row.get(2)?,
            platform: row.get(3)?,
            transcript_json: row.get(4)?,
            summary: row.get(5)?,
            duration_ms: row.get(6)?,
        })
    })?;

    let mut out = Vec::new();
    for row in rows {
        let r = row?;
        let platform = r.platform.as_deref().and_then(platform_from_str);
        let segments: Vec<TranscriptSegment> = r
            .transcript_json
            .as_deref()
            .map(|s| serde_json::from_str(s).unwrap_or_default())
            .unwrap_or_default();
        let word_count: usize = segments
            .iter()
            .map(|s| s.text.split_whitespace().count())
            .sum();
        out.push(MeetingSummary {
            id: r.id,
            started_at: r.started_at,
            ended_at: r.ended_at,
            platform,
            duration_ms: r.duration_ms,
            word_count,
            has_summary: r.summary.is_some(),
        });
    }
    Ok(out)
}

struct RowTuple {
    id: String,
    started_at: String,
    ended_at: Option<String>,
    platform: Option<String>,
    transcript_json: Option<String>,
    summary: Option<String>,
    duration_ms: Option<i64>,
}

fn update_summary_on(conn: &Connection, id: &MeetingId, summary: &str) -> Result<()> {
    let rows = conn.execute(
        "UPDATE meetings SET summary = ?1 WHERE id = ?2",
        params![summary, id],
    )?;
    if rows == 0 {
        return Err(anyhow!("no meeting with id {id}"));
    }
    Ok(())
}

fn update_action_items_on(conn: &Connection, id: &MeetingId, items: &[String]) -> Result<()> {
    let json = serde_json::to_string(items)?;
    let rows = conn.execute(
        "UPDATE meetings SET action_items = ?1 WHERE id = ?2",
        params![json, id],
    )?;
    if rows == 0 {
        return Err(anyhow!("no meeting with id {id}"));
    }
    Ok(())
}

fn update_key_decisions_on(
    conn: &Connection,
    id: &MeetingId,
    decisions: &[String],
) -> Result<()> {
    let json = serde_json::to_string(decisions)?;
    let rows = conn.execute(
        "UPDATE meetings SET key_decisions = ?1 WHERE id = ?2",
        params![json, id],
    )?;
    if rows == 0 {
        return Err(anyhow!("no meeting with id {id}"));
    }
    Ok(())
}

fn delete_meeting_on(conn: &Connection, id: &MeetingId) -> Result<()> {
    let rows = conn.execute("DELETE FROM meetings WHERE id = ?1", params![id])?;
    if rows == 0 {
        return Err(anyhow!("no meeting with id {id}"));
    }
    Ok(())
}

fn row_to_meeting(row: &rusqlite::Row<'_>) -> rusqlite::Result<Meeting> {
    let id: String = row.get(0)?;
    let started_at: String = row.get(1)?;
    let ended_at: Option<String> = row.get(2)?;
    let platform_str: Option<String> = row.get(3)?;
    let audio_path: Option<String> = row.get(4)?;
    let transcript_json: Option<String> = row.get(5)?;
    let summary: Option<String> = row.get(6)?;
    let action_items_json: Option<String> = row.get(7)?;
    let key_decisions_json: Option<String> = row.get(8)?;
    let created_at: String = row.get(9)?;

    let platform = platform_str.as_deref().and_then(platform_from_str);
    let segments: Vec<TranscriptSegment> = transcript_json
        .as_deref()
        .map(|s| serde_json::from_str(s).unwrap_or_default())
        .unwrap_or_default();
    let action_items: Option<Vec<String>> = action_items_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok());
    let key_decisions: Option<Vec<String>> = key_decisions_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok());

    Ok(Meeting {
        id,
        started_at,
        ended_at,
        platform,
        audio_path,
        segments,
        summary,
        action_items,
        key_decisions,
        created_at,
    })
}

fn platform_to_str(p: Platform) -> &'static str {
    match p {
        Platform::Zoom => "zoom",
        Platform::Meet => "meet",
        Platform::Teams => "teams",
        Platform::Slack => "slack",
        Platform::Discord => "discord",
        Platform::Webex => "webex",
        Platform::Manual => "manual",
    }
}

fn platform_from_str(s: &str) -> Option<Platform> {
    Some(match s {
        "zoom" => Platform::Zoom,
        "meet" => Platform::Meet,
        "teams" => Platform::Teams,
        "slack" => Platform::Slack,
        "discord" => Platform::Discord,
        "webex" => Platform::Webex,
        "manual" => Platform::Manual,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        conn
    }

    fn sample_meeting() -> NewMeeting {
        NewMeeting {
            started_at: "2026-04-11T10:00:00.000Z".to_string(),
            ended_at: Some("2026-04-11T10:30:00.000Z".to_string()),
            platform: Some(Platform::Zoom),
            audio_path: Some("/tmp/test.wav".to_string()),
            segments: vec![
                TranscriptSegment {
                    start_ms: 0,
                    end_ms: 1500,
                    text: "hello everyone thanks for joining".to_string(),
                },
                TranscriptSegment {
                    start_ms: 1500,
                    end_ms: 3000,
                    text: "let's get started".to_string(),
                },
            ],
        }
    }

    #[test]
    fn end_to_end_meeting_lifecycle() {
        let conn = fresh();

        let id = save_meeting_on(&conn, sample_meeting()).unwrap();
        assert_eq!(id.len(), 32);

        let fetched = get_meeting_on(&conn, &id).unwrap().unwrap();
        assert_eq!(fetched.id, id);
        assert_eq!(fetched.platform, Some(Platform::Zoom));
        assert_eq!(fetched.segments.len(), 2);
        assert!(fetched.summary.is_none());

        update_summary_on(&conn, &id, "We discussed the Q2 roadmap.").unwrap();
        update_action_items_on(
            &conn,
            &id,
            &["Ship v1".to_string(), "Write docs".to_string()],
        )
        .unwrap();
        update_key_decisions_on(&conn, &id, &["Use Tauri".to_string()]).unwrap();

        let fetched = get_meeting_on(&conn, &id).unwrap().unwrap();
        assert_eq!(fetched.summary.as_deref(), Some("We discussed the Q2 roadmap."));
        assert_eq!(
            fetched.action_items.as_ref().map(|v| v.len()),
            Some(2)
        );
        assert_eq!(fetched.key_decisions.as_ref().map(|v| v.len()), Some(1));

        let list = list_meetings_on(&conn, 10, 0).unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].has_summary);
        assert_eq!(list[0].word_count, 8);
        assert_eq!(list[0].duration_ms, Some(30 * 60 * 1000));

        delete_meeting_on(&conn, &id).unwrap();
        assert!(get_meeting_on(&conn, &id).unwrap().is_none());
        assert_eq!(list_meetings_on(&conn, 10, 0).unwrap().len(), 0);
    }

    #[test]
    fn migrations_are_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        run_migrations(&conn).unwrap();
        run_migrations(&conn).unwrap();
        let version: i64 = conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn update_missing_meeting_errors() {
        let conn = fresh();
        assert!(update_summary_on(&conn, &"nope".to_string(), "x").is_err());
        assert!(delete_meeting_on(&conn, &"nope".to_string()).is_err());
    }

    #[test]
    fn list_newest_first() {
        let conn = fresh();
        let m1 = NewMeeting {
            started_at: "2026-04-01T00:00:00.000Z".to_string(),
            ..sample_meeting()
        };
        let m2 = NewMeeting {
            started_at: "2026-04-10T00:00:00.000Z".to_string(),
            ..sample_meeting()
        };
        let _id1 = save_meeting_on(&conn, m1).unwrap();
        let id2 = save_meeting_on(&conn, m2).unwrap();
        let list = list_meetings_on(&conn, 10, 0).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, id2);
    }
}
