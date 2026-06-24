import { useEffect, useState } from "react";
import { api, type RecordingState } from "../api";

type Phase = "idle" | "recording" | "transcribing";

// Manual record/stop control for the main window. Recording was previously only
// reachable from the tray menu; this drives the same start_recording(manual) /
// stop_recording commands and reflects live state.
export default function RecordButton() {
  const [phase, setPhase] = useState<Phase>("idle");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api
      .recordingState()
      .then((s) => setPhase(s.state === "idle" ? "idle" : s.state))
      .catch(() => {});
    const p = api.onRecordingStateChange((s: RecordingState) => {
      setPhase(s.state === "idle" ? "idle" : s.state);
      setBusy(false);
    });
    return () => {
      p.then((un) => un()).catch(() => {});
    };
  }, []);

  const disabled = busy || phase === "transcribing";

  const onClick = async () => {
    if (disabled) return;
    setBusy(true);
    try {
      if (phase === "recording") {
        await api.stopRecording();
      } else {
        await api.startRecording(true);
      }
    } catch {
      // Failures surface through recording://error -> RecordingStatus banner.
      setBusy(false);
    }
  };

  const recording = phase === "recording";
  const label =
    phase === "transcribing"
      ? "Transcribing…"
      : recording
        ? "Stop"
        : "Record";

  return (
    <button
      onClick={onClick}
      disabled={disabled}
      title={recording ? "Stop recording" : "Start a manual recording"}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 7,
        padding: "5px 12px",
        fontSize: 12,
        fontWeight: 600,
        color: disabled ? "#8a8c94" : "#eaeaea",
        background: recording ? "#7a2a2a" : "#2a2b30",
        border: `1px solid ${recording ? "#a23b3b" : "#3a3b42"}`,
        borderRadius: 6,
        cursor: disabled ? "default" : "pointer",
      }}
    >
      <span
        style={{
          width: 9,
          height: 9,
          borderRadius: recording ? 1 : "50%",
          background: disabled ? "#8a8c94" : "#ff5d5d",
          animation: recording ? "noru-pulse 1.1s ease-in-out infinite" : undefined,
        }}
      />
      {label}
    </button>
  );
}
