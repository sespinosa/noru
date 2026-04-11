import { useEffect, useRef, useState } from "react";
import { api, type AuthStatus } from "../../api";
import { SectionHeader, controlBtnStyle } from "./widgets";

export default function AIFeatures() {
  const [status, setStatus] = useState<AuthStatus | null>(null);
  const [waiting, setWaiting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const pollRef = useRef<number | null>(null);

  const stopPolling = () => {
    if (pollRef.current != null) {
      window.clearInterval(pollRef.current);
      pollRef.current = null;
    }
  };

  const refresh = async () => {
    try {
      const s = await api.authStatus();
      setStatus(s);
      if (s.state === "signed") {
        setWaiting(false);
        stopPolling();
      }
    } catch (e) {
      setError(String(e));
    }
  };

  useEffect(() => {
    refresh();
    const unlistenPromise = api.onAuthStatusChange((s) => {
      setStatus(s);
      if (s.state === "signed") {
        setWaiting(false);
        stopPolling();
      }
    });
    return () => {
      unlistenPromise.then((un) => un()).catch(() => {});
      stopPolling();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const startSignIn = async () => {
    setError(null);
    setWaiting(true);
    try {
      await api.startLogin();
      if (pollRef.current == null) {
        pollRef.current = window.setInterval(refresh, 2000);
      }
    } catch (e) {
      setError(String(e));
      setWaiting(false);
    }
  };

  const signOut = async () => {
    setError(null);
    try {
      await api.signOut();
      setStatus({ state: "signed_out" });
    } catch (e) {
      setError(String(e));
    }
  };

  const cancelWaiting = () => {
    setWaiting(false);
    stopPolling();
  };

  return (
    <div>
      <SectionHeader
        title="AI Features"
        subtitle="Powered by your ChatGPT subscription."
      />
      <div
        style={{
          display: "inline-block",
          marginTop: 6,
          marginBottom: 14,
          fontSize: 10,
          color: "#d2b86e",
          border: "1px solid #55472a",
          padding: "2px 8px",
          borderRadius: 10,
          textTransform: "uppercase",
          letterSpacing: 0.6,
        }}
      >
        experimental
      </div>

      <p
        style={{
          fontSize: 13,
          lineHeight: 1.6,
          color: "#c4c8d1",
          margin: "0 0 10px",
        }}
      >
        noru can summarize your meetings, extract action items, and identify
        key decisions, powered by your ChatGPT subscription.
      </p>
      <p style={{ fontSize: 12, lineHeight: 1.6, color: "#8a8c94", margin: 0 }}>
        This feature uses an unofficial OpenAI sign-in flow that may break. The
        rest of noru works regardless of whether AI is enabled.
      </p>

      <div
        style={{
          marginTop: 20,
          padding: 16,
          border: "1px solid #2a2b30",
          borderRadius: 6,
          background: "#1f2024",
        }}
      >
        {status == null && (
          <p className="placeholder" style={{ margin: 0 }}>
            Checking sign-in…
          </p>
        )}

        {status?.state === "signed_out" && !waiting && (
          <button
            onClick={startSignIn}
            style={{
              ...controlBtnStyle,
              padding: "8px 14px",
              fontSize: 13,
              background: "#4c7dff",
              borderColor: "#4c7dff",
              color: "#ffffff",
            }}
          >
            Sign in with ChatGPT
          </button>
        )}

        {waiting && (
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            <p style={{ margin: 0, fontSize: 13, color: "#c4c8d1" }}>
              Waiting for browser sign-in…
            </p>
            <button
              onClick={cancelWaiting}
              style={{ ...controlBtnStyle, alignSelf: "flex-start" }}
            >
              Cancel
            </button>
          </div>
        )}

        {status?.state === "refreshing" && !waiting && (
          <p className="placeholder" style={{ margin: 0 }}>
            Refreshing session…
          </p>
        )}

        {status?.state === "signed" && (
          <div>
            <div
              style={{
                fontSize: 13,
                marginBottom: 10,
                color: "#7ed69a",
              }}
            >
              ✓ Signed in as {status.account_email}
            </div>
            <button onClick={signOut} style={controlBtnStyle}>
              Sign out
            </button>
          </div>
        )}
      </div>

      {error && (
        <p style={{ marginTop: 12, fontSize: 12, color: "#ff8080" }}>
          {error}{" "}
          <button
            onClick={() => {
              setError(null);
              startSignIn();
            }}
            style={{
              background: "none",
              border: "none",
              color: "#9db8ff",
              cursor: "pointer",
              padding: 0,
              fontSize: 12,
              textDecoration: "underline",
            }}
          >
            try again
          </button>
        </p>
      )}
    </div>
  );
}
