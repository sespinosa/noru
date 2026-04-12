import { useEffect, useState } from "react";
import { api } from "../../api";
import {
  Row,
  Toggle,
  SectionHeader,
  controlBtnStyle,
} from "./widgets";

type Theme = "light" | "dark" | "system";

const THEME_KEY = "noru:settings.general.theme";
const PATH_KEY = "noru:settings.general.transcripts_path";

function isTheme(v: unknown): v is Theme {
  return v === "light" || v === "dark" || v === "system";
}

export default function General() {
  const [autoStart, setAutoStart] = useState<boolean | null>(null);
  const [theme, setTheme] = useState<Theme>(() => {
    const v = window.localStorage.getItem(THEME_KEY);
    return isTheme(v) ? v : "system";
  });
  const [transcriptsPath, setTranscriptsPath] = useState("~/.noru/transcripts/");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api
      .getAutoStart()
      .then(setAutoStart)
      .catch((e) => {
        setAutoStart(false);
        setError(String(e));
      });
  }, []);

  useEffect(() => {
    api.getPreference<string>(PATH_KEY).then((v) => {
      if (typeof v === "string" && v) setTranscriptsPath(v);
    }).catch(() => {});
  }, []);

  useEffect(() => {
    window.localStorage.setItem(THEME_KEY, theme);
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  const toggleAutoStart = async () => {
    if (autoStart == null) return;
    const next = !autoStart;
    setError(null);
    try {
      await api.setAutoStart(next);
      setAutoStart(next);
    } catch (e) {
      setError(String(e));
    }
  };


  return (
    <div>
      <SectionHeader title="General" />

      <Row
        label="Auto-start with Windows"
        hint="Launch noru in the tray when you sign in."
      >
        <Toggle
          checked={autoStart ?? false}
          disabled={autoStart == null}
          onChange={toggleAutoStart}
        />
      </Row>

      <Row
        label="Transcripts folder"
        hint="Where recordings and transcripts are stored."
      >
        <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
          <code
            style={{
              fontSize: 12,
              color: "#c4c8d1",
              background: "#1a1b1e",
              padding: "3px 6px",
              borderRadius: 3,
              border: "1px solid #2a2b30",
            }}
          >
            {transcriptsPath}
          </code>
          <button
            onClick={async () => {
              try {
                const folder = await api.chooseFolder("Choose transcripts folder");
                if (folder) {
                  setTranscriptsPath(folder);
                  await api.setPreference(PATH_KEY, folder);
                }
              } catch (e) {
                setError(String(e));
              }
            }}
            style={controlBtnStyle}
          >
            Choose…
          </button>
        </div>
      </Row>

      <Row label="Theme" hint="Override the system appearance.">
        <div style={{ display: "flex", gap: 14 }}>
          {(["light", "dark", "system"] as Theme[]).map((t) => (
            <label
              key={t}
              style={{
                fontSize: 13,
                display: "flex",
                gap: 5,
                alignItems: "center",
                cursor: "pointer",
              }}
            >
              <input
                type="radio"
                name="theme"
                checked={theme === t}
                onChange={() => setTheme(t)}
              />
              {t[0].toUpperCase() + t.slice(1)}
            </label>
          ))}
        </div>
      </Row>

      {error && (
        <p style={{ marginTop: 12, fontSize: 11, color: "#ff8080" }}>{error}</p>
      )}
    </div>
  );
}
