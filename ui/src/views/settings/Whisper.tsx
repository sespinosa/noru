import { useEffect, useState } from "react";
import { api, type ModelDownloadProgress } from "../../api";
import { Row, SectionHeader, inputStyle } from "./widgets";

const MODELS: { key: string; label: string; size: string }[] = [
  { key: "tiny", label: "tiny", size: "75 MB" },
  { key: "base", label: "base", size: "142 MB" },
  { key: "small", label: "small", size: "466 MB" },
  { key: "medium", label: "medium", size: "1.5 GB" },
  { key: "large-v3", label: "large-v3", size: "2.9 GB" },
  { key: "large-v3-turbo", label: "large-v3 turbo", size: "809 MB" },
];

const LANGUAGES: { code: string; label: string }[] = [
  { code: "auto", label: "Auto-detect" },
  { code: "en", label: "English" },
  { code: "es", label: "Spanish" },
  { code: "fr", label: "French" },
  { code: "de", label: "German" },
  { code: "pt", label: "Portuguese" },
  { code: "it", label: "Italian" },
  { code: "ja", label: "Japanese" },
  { code: "zh", label: "Chinese" },
  { code: "ko", label: "Korean" },
  { code: "other", label: "Other…" },
];

const LS_MODEL = "noru:settings.whisper.model";
const LS_LANG = "noru:settings.whisper.language";
const LS_LANG_OTHER = "noru:settings.whisper.language_other";

const MODEL_KEYS = new Set(MODELS.map((m) => m.key));
const LANG_CODES = new Set(LANGUAGES.map((l) => l.code));

export default function Whisper() {
  const [model, setModel] = useState("tiny");
  const [language, setLanguage] = useState("auto");
  const [otherLang, setOtherLang] = useState<string>(
    () => window.localStorage.getItem(LS_LANG_OTHER) ?? "",
  );
  const [progress, setProgress] = useState<ModelDownloadProgress | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.getPreference<string>(LS_MODEL).then((v) => {
      if (typeof v === "string" && MODEL_KEYS.has(v)) setModel(v);
    }).catch(() => {});
    api.getPreference<string>(LS_LANG).then((v) => {
      if (typeof v === "string" && LANG_CODES.has(v)) setLanguage(v);
    }).catch(() => {});
  }, []);

  useEffect(() => {
    const unlisten = api.onModelDownloadProgress((p) => {
      setProgress(p);
      if (p.percent >= 100) {
        window.setTimeout(() => setProgress(null), 1000);
      }
    });
    return () => {
      unlisten.then((un) => un()).catch(() => {});
    };
  }, []);

  useEffect(() => {
    window.localStorage.setItem(LS_LANG_OTHER, otherLang);
  }, [otherLang]);

  const onSelectModel = async (m: string) => {
    setModel(m);
    setError(null);
    api.setPreference(LS_MODEL, m).catch(() => {});
    try {
      await api.downloadModel(m);
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div>
      <SectionHeader
        title="Whisper"
        subtitle="Local speech-to-text engine. Larger models are more accurate but slower."
      />

      <Row label="Model" hint="Downloaded on first use if missing.">
        <select
          value={model}
          onChange={(e) => onSelectModel(e.target.value)}
          style={inputStyle}
        >
          {MODELS.map((m) => (
            <option key={m.key} value={m.key}>
              {m.label} — {m.size}
            </option>
          ))}
        </select>
      </Row>

      {progress && (
        <div style={{ padding: "8px 0", fontSize: 12, color: "#8a8c94" }}>
          Downloading {progress.model}: {Math.round(progress.percent)}%
          <div
            style={{
              marginTop: 4,
              height: 4,
              background: "#2a2b30",
              borderRadius: 2,
              overflow: "hidden",
            }}
          >
            <div
              style={{
                height: "100%",
                width: `${progress.percent}%`,
                background: "#4c7dff",
                transition: "width 0.2s",
              }}
            />
          </div>
        </div>
      )}

      <Row label="Language" hint="Leave on auto-detect unless Whisper is guessing wrong.">
        <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
          <select
            value={language}
            onChange={(e) => {
              const v = e.target.value;
              setLanguage(v);
              api.setPreference(LS_LANG, v).catch(() => {});
            }}
            style={inputStyle}
          >
            {LANGUAGES.map((l) => (
              <option key={l.code} value={l.code}>
                {l.label}
              </option>
            ))}
          </select>
          {language === "other" && (
            <input
              type="text"
              placeholder="e.g. ru"
              value={otherLang}
              onChange={(e) => setOtherLang(e.target.value)}
              style={{ ...inputStyle, width: 120 }}
            />
          )}
        </div>
      </Row>

      {error && (
        <p style={{ marginTop: 12, fontSize: 11, color: "#ff8080" }}>
          {error}{" "}
          <button
            onClick={() => onSelectModel(model)}
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
