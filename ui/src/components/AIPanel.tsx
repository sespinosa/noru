import { useEffect, useState } from "react";
import type { ReactNode } from "react";
import { Link } from "react-router-dom";
import { api, type AuthStatus, type Meeting } from "../api";

type FeatureKey = "summary" | "action_items" | "key_decisions";

interface Props {
  meeting: Meeting;
  onUpdated?: (m: Meeting) => void;
}

export default function AIPanel({ meeting, onUpdated }: Props) {
  const [status, setStatus] = useState<AuthStatus | null>(null);
  const [loading, setLoading] = useState<FeatureKey | null>(null);
  const [errors, setErrors] = useState<Partial<Record<FeatureKey, string>>>({});

  useEffect(() => {
    let cancelled = false;
    api
      .authStatus()
      .then((s) => {
        if (!cancelled) setStatus(s);
      })
      .catch(() => {
        if (!cancelled) setStatus({ state: "signed_out" });
      });
    const unlistenPromise = api.onAuthStatusChange((s) => setStatus(s));
    return () => {
      cancelled = true;
      unlistenPromise.then((un) => un()).catch(() => {});
    };
  }, []);

  const run = async (key: FeatureKey) => {
    setLoading(key);
    setErrors((e) => ({ ...e, [key]: undefined }));
    try {
      if (key === "summary") {
        const summary = await api.aiSummarize(meeting.id);
        onUpdated?.({ ...meeting, summary });
      } else if (key === "action_items") {
        const action_items = await api.aiExtractActionItems(meeting.id);
        onUpdated?.({ ...meeting, action_items });
      } else {
        const key_decisions = await api.aiExtractKeyDecisions(meeting.id);
        onUpdated?.({ ...meeting, key_decisions });
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setErrors((prev) => ({ ...prev, [key]: msg }));
    } finally {
      setLoading(null);
    }
  };

  const panelStyle: React.CSSProperties = {
    padding: 12,
    border: "1px solid #2a2b30",
    borderRadius: 6,
    background: "#1f2024",
  };

  return (
    <div style={panelStyle}>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: 10,
        }}
      >
        <h3 style={{ margin: 0, fontSize: 13, fontWeight: 600 }}>AI features</h3>
        <span
          style={{
            fontSize: 9,
            color: "#d2b86e",
            border: "1px solid #55472a",
            padding: "1px 6px",
            borderRadius: 10,
            textTransform: "uppercase",
            letterSpacing: 0.6,
          }}
        >
          experimental
        </span>
      </div>

      {status == null && <p className="placeholder" style={{ margin: 0 }}>Checking…</p>}

      {status?.state === "signed_out" && (
        <p className="placeholder" style={{ margin: 0, fontSize: 12, lineHeight: 1.5 }}>
          <Link to="/settings" style={{ color: "#9db8ff" }}>
            Sign in to ChatGPT in Settings
          </Link>{" "}
          to enable AI features.
        </p>
      )}

      {status?.state === "refreshing" && (
        <p className="placeholder" style={{ margin: 0 }}>Refreshing session…</p>
      )}

      {status?.state === "signed" && (
        <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
          <FeatureBlock
            label="Summarize"
            cached={meeting.summary}
            loading={loading === "summary"}
            error={errors.summary}
            onRun={() => run("summary")}
            render={(v) => (
              <p
                style={{
                  margin: 0,
                  fontSize: 12,
                  lineHeight: 1.5,
                  whiteSpace: "pre-wrap",
                }}
              >
                {v}
              </p>
            )}
          />
          <FeatureBlock
            label="Action items"
            cached={arrOrNull(meeting.action_items)}
            loading={loading === "action_items"}
            error={errors.action_items}
            onRun={() => run("action_items")}
            render={(items) => (
              <ul
                style={{
                  margin: 0,
                  paddingLeft: 16,
                  fontSize: 12,
                  lineHeight: 1.5,
                }}
              >
                {items.map((it, i) => (
                  <li key={i}>{it}</li>
                ))}
              </ul>
            )}
          />
          <FeatureBlock
            label="Key decisions"
            cached={arrOrNull(meeting.key_decisions)}
            loading={loading === "key_decisions"}
            error={errors.key_decisions}
            onRun={() => run("key_decisions")}
            render={(items) => (
              <ul
                style={{
                  margin: 0,
                  paddingLeft: 16,
                  fontSize: 12,
                  lineHeight: 1.5,
                }}
              >
                {items.map((it, i) => (
                  <li key={i}>{it}</li>
                ))}
              </ul>
            )}
          />
        </div>
      )}
    </div>
  );
}

function arrOrNull(a: string[] | null): string[] | null {
  if (a == null || a.length === 0) return null;
  return a;
}

interface FeatureBlockProps<T> {
  label: string;
  cached: T | null;
  loading: boolean;
  error: string | undefined;
  onRun: () => void;
  render: (v: T) => ReactNode;
}

function FeatureBlock<T>({
  label,
  cached,
  loading,
  error,
  onRun,
  render,
}: FeatureBlockProps<T>) {
  const hasCached = cached != null;
  return (
    <div>
      <button
        onClick={onRun}
        disabled={loading}
        style={{
          width: "100%",
          padding: "6px 10px",
          background: "#2a2b30",
          color: "#eaeaea",
          border: "1px solid #3a3b42",
          borderRadius: 4,
          fontSize: 12,
          cursor: loading ? "default" : "pointer",
          textAlign: "left",
          opacity: loading ? 0.7 : 1,
        }}
      >
        {loading
          ? `Working on ${label.toLowerCase()}…`
          : hasCached
            ? `Regenerate ${label.toLowerCase()}`
            : label}
      </button>
      {hasCached && !loading && (
        <div style={{ marginTop: 6 }}>{render(cached as T)}</div>
      )}
      {error && (
        <div style={{ marginTop: 6, fontSize: 11, color: "#ff8080" }}>
          {error}
          <div style={{ marginTop: 2, color: "#8a8c94" }}>
            If this keeps failing, report on GitHub.
          </div>
        </div>
      )}
    </div>
  );
}
