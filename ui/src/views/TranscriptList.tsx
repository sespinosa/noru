import { useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { api, type MeetingSummary, type Platform } from "../api";

const PLATFORM_LABELS: Record<Platform, string> = {
  zoom: "Zoom",
  meet: "Meet",
  teams: "Teams",
  slack: "Slack",
  discord: "Discord",
  webex: "Webex",
  manual: "Manual",
};

const PLATFORM_ICONS: Record<Platform, string> = {
  zoom: "🎥",
  meet: "📹",
  teams: "💼",
  slack: "💬",
  discord: "🎧",
  webex: "📡",
  manual: "🎙",
};

function formatRelative(iso: string): string {
  const d = new Date(iso);
  const diff = Date.now() - d.getTime();
  if (Number.isNaN(diff)) return "";
  const mins = Math.floor(diff / 60_000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins} min ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d ago`;
  return d.toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

function formatDuration(ms: number | null): string {
  if (ms == null) return "—";
  const totalSec = Math.max(0, Math.floor(ms / 1000));
  const m = Math.floor(totalSec / 60);
  const s = totalSec % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export default function TranscriptList() {
  const { id: selectedId } = useParams<{ id: string }>();
  const [meetings, setMeetings] = useState<MeetingSummary[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    const load = () => {
      api
        .listMeetings(100, 0)
        .then((ms) => {
          if (!cancelled) setMeetings(ms);
        })
        .catch((e) => {
          if (!cancelled) {
            setMeetings([]);
            setError(String(e));
          }
        });
    };
    load();
    const unlistenPromise = api.onRecordingStateChange((s) => {
      if (s.state === "idle") load();
    });
    return () => {
      cancelled = true;
      unlistenPromise.then((un) => un()).catch(() => {});
    };
  }, []);

  if (meetings == null) {
    return (
      <div style={{ padding: 12 }}>
        <p className="placeholder">Loading…</p>
      </div>
    );
  }

  if (meetings.length === 0) {
    return (
      <div style={{ padding: 12 }}>
        <p className="placeholder">
          No recordings yet. Start a meeting and noru will record it
          automatically.
        </p>
        {error && (
          <p style={{ fontSize: 11, color: "#ff8080", marginTop: 8 }}>
            {error}
          </p>
        )}
      </div>
    );
  }

  return (
    <ul style={{ listStyle: "none", margin: 0, padding: 0 }}>
      {meetings.map((m) => {
        const isActive = m.id === selectedId;
        const platform: Platform = m.platform ?? "manual";
        return (
          <li key={m.id}>
            <Link
              to={`/transcripts/${m.id}`}
              style={{
                display: "block",
                padding: "10px 12px",
                borderBottom: "1px solid #23242a",
                textDecoration: "none",
                color: "#eaeaea",
                background: isActive ? "#2a2b30" : "transparent",
              }}
            >
              <div
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  alignItems: "center",
                  fontSize: 12,
                }}
              >
                <span>
                  <span style={{ marginRight: 4 }}>
                    {PLATFORM_ICONS[platform]}
                  </span>
                  {PLATFORM_LABELS[platform]}
                </span>
                <span style={{ color: "#8a8c94", fontSize: 11 }}>
                  {formatRelative(m.started_at)}
                </span>
              </div>
              <div
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  marginTop: 4,
                  fontSize: 11,
                  color: "#8a8c94",
                }}
              >
                <span>
                  {formatDuration(m.duration_ms)} · {m.word_count} words
                </span>
                {m.has_summary && (
                  <span
                    style={{
                      color: "#7ed69a",
                      border: "1px solid #2f5a3a",
                      borderRadius: 8,
                      padding: "0 5px",
                      fontSize: 10,
                    }}
                  >
                    AI ✓
                  </span>
                )}
              </div>
            </Link>
          </li>
        );
      })}
    </ul>
  );
}
