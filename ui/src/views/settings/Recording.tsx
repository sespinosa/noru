import { useEffect, useState } from "react";
import { api, type AudioDevice, type Platform } from "../../api";
import {
  Row,
  Toggle,
  SectionHeader,
  inputStyle,
  lsGet,
  lsSet,
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
  // TODO(phase-3): swap to api.getAutoDetect() / api.setAutoDetect()
  const [autoDetect, setAutoDetect] = useState<boolean>(() =>
    lsGet(LS_AUTO, true),
  );
  // TODO(phase-3): swap to api.getEnabledPlatforms() / api.setEnabledPlatforms()
  const [enabled, setEnabled] = useState<Platform[]>(() => {
    const stored = lsGet<Platform[]>(LS_ENABLED, PLATFORMS.map((p) => p.key));
    return Array.isArray(stored) ? stored : PLATFORMS.map((p) => p.key);
  });
  const [devices, setDevices] = useState<AudioDevice[]>([]);
  // TODO(phase-3): swap to api.getAudioInputDevice() / api.setAudioInputDevice()
  const [selectedDevice, setSelectedDevice] = useState<string>(() =>
    lsGet(LS_DEVICE, ""),
  );
  // TODO(phase-3): swap to api.getCaptureSystemAudio() / api.setCaptureSystemAudio()
  const [systemAudio, setSystemAudio] = useState<boolean>(() =>
    lsGet(LS_LOOPBACK, true),
  );
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api
      .listAudioInputDevices()
      .then(setDevices)
      .catch((e) => setError(String(e)));
  }, []);

  useEffect(() => lsSet(LS_AUTO, autoDetect), [autoDetect]);
  useEffect(() => lsSet(LS_ENABLED, enabled), [enabled]);
  useEffect(() => lsSet(LS_DEVICE, selectedDevice), [selectedDevice]);
  useEffect(() => lsSet(LS_LOOPBACK, systemAudio), [systemAudio]);

  const togglePlatform = (p: Platform) => {
    setEnabled((cur) =>
      cur.includes(p) ? cur.filter((x) => x !== p) : [...cur, p],
    );
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
          onChange={() => setAutoDetect((v) => !v)}
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
          onChange={(e) => setSelectedDevice(e.target.value)}
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
          onChange={() => setSystemAudio((v) => !v)}
        />
      </Row>

      {error && (
        <p style={{ marginTop: 12, fontSize: 11, color: "#ff8080" }}>{error}</p>
      )}
    </div>
  );
}
