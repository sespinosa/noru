import { useEffect, useState } from "react";
import { api, type AudioDevice, type Platform } from "../../api";
import {
  Row,
  Toggle,
  SectionHeader,
  inputStyle,
} from "./widgets";

const PLATFORMS: { key: Platform; label: string }[] = [
  { key: "zoom", label: "Zoom" },
  { key: "teams", label: "Microsoft Teams" },
  { key: "meet", label: "Google Meet" },
  { key: "slack", label: "Slack" },
  { key: "discord", label: "Discord" },
  { key: "webex", label: "Webex" },
];

const LS_AUTO = "noru:settings.recording.auto_detect";
const LS_ENABLED = "noru:settings.recording.enabled_platforms";
const LS_DEVICE = "noru:settings.recording.device";
const LS_LOOPBACK = "noru:settings.recording.system_audio";

export default function Recording() {
  const [autoDetect, setAutoDetect] = useState(true);
  const [enabled, setEnabled] = useState<Platform[]>(
    PLATFORMS.map((p) => p.key),
  );
  const [devices, setDevices] = useState<AudioDevice[]>([]);
  const [selectedDevice, setSelectedDevice] = useState("");
  const [systemAudio, setSystemAudio] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.getPreference<boolean>(LS_AUTO).then((v) => {
      if (typeof v === "boolean") setAutoDetect(v);
    }).catch(() => {});
    api.getPreference<Platform[]>(LS_ENABLED).then((v) => {
      if (Array.isArray(v)) setEnabled(v);
    }).catch(() => {});
    api.getPreference<string>(LS_DEVICE).then((v) => {
      if (typeof v === "string") setSelectedDevice(v);
    }).catch(() => {});
    api.getPreference<boolean>(LS_LOOPBACK).then((v) => {
      if (typeof v === "boolean") setSystemAudio(v);
    }).catch(() => {});
  }, []);

  const loadDevices = () => {
    setError(null);
    api
      .listAudioInputDevices()
      .then(setDevices)
      .catch((e) => setError(String(e)));
  };

  useEffect(loadDevices, []);

  const togglePlatform = (p: Platform) => {
    setEnabled((cur) => {
      const next = cur.includes(p) ? cur.filter((x) => x !== p) : [...cur, p];
      api.setPreference(LS_ENABLED, next).catch(() => {});
      return next;
    });
  };

  return (
    <div>
      <SectionHeader title="Recording" />

      <Row
        label="Auto-detect meetings"
        hint="When off, noru only records when you start it manually from the tray."
      >
        <Toggle
          checked={autoDetect}
          onChange={() => {
            const next = !autoDetect;
            setAutoDetect(next);
            api.setPreference(LS_AUTO, next).catch(() => {});
          }}
        />
      </Row>

      <Row
        label="Platforms to auto-detect"
        hint="Only meetings on the checked platforms will trigger auto-recording."
      >
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "1fr 1fr",
            gap: "4px 20px",
          }}
        >
          {PLATFORMS.map((p) => (
            <label
              key={p.key}
              style={{
                display: "flex",
                gap: 6,
                alignItems: "center",
                fontSize: 12,
                color: autoDetect ? "#eaeaea" : "#6a6c74",
                cursor: autoDetect ? "pointer" : "default",
              }}
            >
              <input
                type="checkbox"
                checked={enabled.includes(p.key)}
                onChange={() => togglePlatform(p.key)}
                disabled={!autoDetect}
              />
              {p.label}
            </label>
          ))}
        </div>
      </Row>

      <Row
        label="Audio input device"
        hint="Microphone used for capturing your voice."
      >
        <select
          value={selectedDevice}
          onChange={(e) => {
            const v = e.target.value;
            setSelectedDevice(v);
            api.setPreference(LS_DEVICE, v).catch(() => {});
          }}
          style={inputStyle}
        >
          <option value="">System default</option>
          {devices.map((d) => (
            <option key={d.name} value={d.name}>
              {d.name}
              {d.is_default ? " (default)" : ""}
            </option>
          ))}
        </select>
      </Row>

      <Row
        label="Capture system audio"
        hint="Uses WASAPI loopback to capture other meeting participants."
      >
        <Toggle
          checked={systemAudio}
          onChange={() => {
            const next = !systemAudio;
            setSystemAudio(next);
            api.setPreference(LS_LOOPBACK, next).catch(() => {});
          }}
        />
      </Row>

      {error && (
        <p style={{ marginTop: 12, fontSize: 11, color: "#ff8080" }}>
          {error}{" "}
          <button
            onClick={loadDevices}
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
