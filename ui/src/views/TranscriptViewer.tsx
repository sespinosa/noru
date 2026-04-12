import { useEffect, useMemo, useState } from "react";
import type { CSSProperties } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { api, type Meeting } from "../api";
import AIPanel from "../components/AIPanel";

function formatTime(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  const m = Math.floor(s / 60);
  const sec = s % 60;
  return `${m}:${sec.toString().padStart(2, "0")}`;
}

function titleFor(m: Meeting): string {
  const date = new Date(m.started_at);
  const platform = m.platform
    ? m.platform[0].toUpperCase() + m.platform.slice(1)
    : "Recording";
  const when = Number.isNaN(date.getTime())
    ? ""
    : date.toLocaleString(undefined, {
        month: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      });
  return when ? `${platform} · ${when}` : platform;
}

function durationMs(m: Meeting): number | null {
  if (!m.ended_at) return null;
  const start = new Date(m.started_at).getTime();
  const end = new Date(m.ended_at).getTime();
  if (Number.isNaN(start) || Number.isNaN(end)) return null;
  return end - start;
}

function toMarkdown(m: Meeting): string {
  const header = `# ${titleFor(m)}\n\n`;
  const body = m.segments
    .map((s) => `**[${formatTime(s.start_ms)}]** ${s.text}`)
    .join("\n\n");
  const extras: string[] = [];
  if (m.summary) extras.push(`\n\n## Summary\n\n${m.summary}`);
  if (m.action_items && m.action_items.length > 0) {
    extras.push(
      `\n\n## Action items\n\n${m.action_items.map((i) => `- ${i}`).join("\n")}`,
    );
  }
  if (m.key_decisions && m.key_decisions.length > 0) {
    extras.push(
      `\n\n## Key decisions\n\n${m.key_decisions.map((i) => `- ${i}`).join("\n")}`,
    );
  }
  return header + body + extras.join("");
}

const btnStyle: CSSProperties = {
  background: "#2a2b30",
  color: "#eaeaea",
  border: "1px solid #3a3b42",
  borderRadius: 4,
  padding: "4px 10px",
  fontSize: 11,
  cursor: "pointer",
};

export default function TranscriptViewer() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [meeting, setMeeting] = useState<Meeting | null | undefined>(undefined);
  const [query, setQuery] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);

  useEffect(() => {
    if (!id) {
      setMeeting(null);
      return;
    }
    let cancelled = false;
    setMeeting(undefined);
    setError(null);
    api
      .getMeeting(id)
      .then((m) => {
        if (!cancelled) setMeeting(m);
      })
      .catch((e) => {
        if (!cancelled) {
          setMeeting(null);
          setError(String(e));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [id]);

  const retryLoad = () => {
    if (!id) return;
    setMeeting(undefined);
    setError(null);
    api.getMeeting(id).then(setMeeting).catch((e) => {
      setMeeting(null);
      setError(String(e));
    });
  };

  useEffect(() => {
    if (!toast) return;
    const t = window.setTimeout(() => setToast(null), 1800);
    return () => window.clearTimeout(t);
  }, [toast]);

  const filtered = useMemo(() => {
    if (!meeting) return [];
    if (!query.trim()) return meeting.segments;
    const q = query.toLowerCase();
    return meeting.segments.filter((s) => s.text.toLowerCase().includes(q));
  }, [meeting, query]);

  if (!id) {
    return (
      <p className="placeholder">
        Select a recording from the sidebar to view its transcript.
      </p>
    );
  }
  if (meeting === undefined) {
    return <p className="placeholder">Loading transcript…</p>;
  }
  if (meeting === null) {
    return (
      <div>
        <p className="placeholder">Transcript not found.</p>
        {error && (
          <p style={{ color: "#ff8080", fontSize: 11 }}>
            {error}{" "}
            <button
              onClick={retryLoad}
              style={{
                background: "none",
                border: "none",
                color: "#9db8ff",
                cursor: "pointer",
                padding: 0,
                fontSize: 11,
                textDecoration: "underline",
              }}
            >
              Retry
            </button>
          </p>
        )}
      </div>
    );
  }

  const onCopy = async () => {
    try {
      await navigator.clipboard.writeText(
        meeting.segments.map((s) => s.text).join("\n"),
      );
      setToast("Copied transcript");
    } catch (e) {
      setError(String(e));
    }
  };
  const onExport = async () => {
    try {
      await navigator.clipboard.writeText(toMarkdown(meeting));
      setToast("Copied as Markdown");
    } catch (e) {
      setError(String(e));
    }
  };
  const onDelete = async () => {
    if (!window.confirm("Delete this recording permanently?")) return;
    try {
      await api.deleteMeeting(meeting.id);
      navigate("/transcripts");
    } catch (e) {
      setError(String(e));
    }
  };

  const dms = durationMs(meeting);

  return (
    <div style={{ display: "flex", gap: 16, height: "100%" }}>
      <div
        style={{
          flex: 1,
          minWidth: 0,
          display: "flex",
          flexDirection: "column",
        }}
      >
        <header style={{ marginBottom: 12 }}>
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "flex-start",
              gap: 8,
            }}
          >
            <div style={{ minWidth: 0 }}>
              <h2
                style={{
                  margin: 0,
                  fontSize: 16,
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                  whiteSpace: "nowrap",
                }}
              >
                {titleFor(meeting)}
              </h2>
              <div
                style={{
                  marginTop: 4,
                  color: "#8a8c94",
                  fontSize: 12,
                }}
              >
                {new Date(meeting.started_at).toLocaleString()}
                {dms != null && ` · ${formatTime(dms)}`}
                {meeting.platform && ` · ${meeting.platform}`}
              </div>
            </div>
            <div style={{ display: "flex", gap: 6, flexShrink: 0 }}>
              <button onClick={onCopy} style={btnStyle}>
                Copy
              </button>
              <button onClick={onExport} style={btnStyle}>
                Export MD
              </button>
              <button
                onClick={onDelete}
                style={{ ...btnStyle, color: "#ff8080" }}
              >
                Delete
              </button>
            </div>
          </div>
          <input
            type="search"
            placeholder="Search transcript…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            style={{
              marginTop: 10,
              width: "100%",
              padding: "6px 10px",
              background: "#1a1b1e",
              color: "#eaeaea",
              border: "1px solid #2a2b30",
              borderRadius: 4,
              fontSize: 13,
            }}
          />
          {toast && (
            <div
              style={{
                marginTop: 8,
                fontSize: 11,
                color: "#7ed69a",
              }}
            >
              {toast}
            </div>
          )}
          {error && (
            <div
              style={{
                marginTop: 8,
                fontSize: 11,
                color: "#ff8080",
              }}
            >
              {error}{" "}
              <button
                onClick={retryLoad}
                style={{
                  background: "none",
                  border: "none",
                  color: "#9db8ff",
                  cursor: "pointer",
                  padding: 0,
                  fontSize: 11,
                  textDecoration: "underline",
                }}
              >
                Retry
              </button>
            </div>
          )}
        </header>
        <div
          style={{
            flex: 1,
            overflowY: "auto",
            paddingRight: 8,
            minHeight: 0,
          }}
        >
          {filtered.length === 0 ? (
            <p className="placeholder">
              {meeting.segments.length === 0
                ? "Transcript is empty."
                : "No segments match your search."}
            </p>
          ) : (
            filtered.map((s, i) => (
              <div
                key={i}
                style={{
                  display: "flex",
                  gap: 10,
                  marginBottom: 8,
                  fontSize: 13,
                  lineHeight: 1.5,
                }}
              >
                <button
                  onClick={() => {
                    /* audio scrubbing is out of v1 scope */
                  }}
                  title={`Seek to ${formatTime(s.start_ms)}`}
                  style={{
                    background: "transparent",
                    border: "none",
                    color: "#8a8c94",
                    padding: 0,
                    fontFamily: "monospace",
                    fontSize: 11,
                    cursor: "pointer",
                    flexShrink: 0,
                    width: 42,
                    textAlign: "right",
                  }}
                >
                  {formatTime(s.start_ms)}
                </button>
                <p style={{ margin: 0, flex: 1 }}>{s.text}</p>
              </div>
            ))
          )}
        </div>
      </div>
      <aside
        style={{
          width: 300,
          flexShrink: 0,
          overflowY: "auto",
        }}
      >
        <AIPanel meeting={meeting} onUpdated={(m) => setMeeting(m)} />
      </aside>
    </div>
  );
}
