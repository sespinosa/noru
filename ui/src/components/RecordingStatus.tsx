import { useEffect, useRef, useState } from "react";
import { api, type RecordingState } from "../api";
import { playDone, playError } from "../lib/sound";

type Phase = "idle" | "recording" | "transcribing";

// A live, app-global banner reflecting the recording lifecycle. Without it the
// window gives no feedback between Stop and the transcript appearing (Whisper
// runs for several seconds on CPU). Also plays a synthesized cue on completion.
export default function RecordingStatus() {
  const [phase, setPhase] = useState<Phase>("idle");
  const [error, setError] = useState<string | null>(null);
  const [justDone, setJustDone] = useState(false);
  const prevPhase = useRef<Phase>("idle");
  const doneTimer = useRef<number | undefined>(undefined);

  useEffect(() => {
    const statePromise = api.onRecordingStateChange((s: RecordingState) => {
      const next: Phase = s.state === "idle" ? "idle" : s.state;
      // transcribing -> idle means a transcript just finished successfully.
      if (prevPhase.current === "transcribing" && next === "idle") {
        playDone();
        setJustDone(true);
        setError(null);
        window.clearTimeout(doneTimer.current);
        doneTimer.current = window.setTimeout(() => setJustDone(false), 3000);
      }
      if (next !== "idle") setError(null);
      prevPhase.current = next;
      setPhase(next);
    });
    const errPromise = api.onRecordingError((msg) => {
      playError();
      setError(msg);
      setJustDone(false);
      // An error aborts the in-flight recording; the backend also emits idle.
      prevPhase.current = "idle";
      setPhase("idle");
    });
    return () => {
      statePromise.then((un) => un()).catch(() => {});
      errPromise.then((un) => un()).catch(() => {});
      window.clearTimeout(doneTimer.current);
    };
  }, []);

  let content: { dot: string; text: string; color: string } | null = null;
  if (error) {
    content = { dot: "#ff6b6b", text: `Recording failed: ${error}`, color: "#ff8080" };
  } else if (phase === "recording") {
    content = { dot: "#ff5d5d", text: "Recording…", color: "#ffb3b3" };
  } else if (phase === "transcribing") {
    content = {
      dot: "#ffce54",
      text: "Transcribing… (a few seconds on CPU)",
      color: "#ffe08a",
    };
  } else if (justDone) {
    content = { dot: "#7ed69a", text: "Transcript ready", color: "#9ee6b4" };
  }

  if (!content) return null;

  const pulsing = phase === "recording" || phase === "transcribing";
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 8,
        padding: "6px 12px",
        fontSize: 12,
        color: content.color,
        background: "#1b1c21",
        borderBottom: "1px solid #23242a",
      }}
    >
      <span
        style={{
          width: 8,
          height: 8,
          borderRadius: "50%",
          background: content.dot,
          animation: pulsing ? "noru-pulse 1.1s ease-in-out infinite" : undefined,
        }}
      />
      <span>{content.text}</span>
      <style>{`@keyframes noru-pulse { 0%,100% { opacity: 1 } 50% { opacity: 0.25 } }`}</style>
    </div>
  );
}
