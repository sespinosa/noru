import { useEffect, useState } from "react";
import General from "./settings/General";
import Recording from "./settings/Recording";
import Whisper from "./settings/Whisper";
import AIFeatures from "./settings/AIFeatures";

type Section = "general" | "recording" | "whisper" | "ai";

const SECTIONS: { key: Section; label: string }[] = [
  { key: "general", label: "General" },
  { key: "recording", label: "Recording" },
  { key: "whisper", label: "Whisper" },
  { key: "ai", label: "AI Features" },
];

const STORAGE_KEY = "noru:settings.last_section";

function isSection(v: unknown): v is Section {
  return v === "general" || v === "recording" || v === "whisper" || v === "ai";
}

export default function Settings() {
  const [section, setSection] = useState<Section>(() => {
    const saved = window.localStorage.getItem(STORAGE_KEY);
    return isSection(saved) ? saved : "general";
  });

  useEffect(() => {
    window.localStorage.setItem(STORAGE_KEY, section);
  }, [section]);

  return (
    <div style={{ display: "flex", gap: 24, height: "100%" }}>
      <nav
        style={{
          width: 180,
          flexShrink: 0,
          display: "flex",
          flexDirection: "column",
          gap: 2,
        }}
      >
        {SECTIONS.map((s) => {
          const active = section === s.key;
          return (
            <button
              key={s.key}
              onClick={() => setSection(s.key)}
              style={{
                background: active ? "#2a2b30" : "transparent",
                color: active ? "#eaeaea" : "#9aa0ad",
                border: "none",
                padding: "8px 10px",
                textAlign: "left",
                fontSize: 13,
                borderRadius: 4,
                cursor: "pointer",
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
              }}
            >
              <span>{s.label}</span>
              {s.key === "ai" && (
                <span style={{ fontSize: 9, color: "#d2b86e" }}>
                  experimental
                </span>
              )}
            </button>
          );
        })}
      </nav>
      <div style={{ flex: 1, minWidth: 0, overflowY: "auto" }}>
        {section === "general" && <General />}
        {section === "recording" && <Recording />}
        {section === "whisper" && <Whisper />}
        {section === "ai" && <AIFeatures />}
      </div>
    </div>
  );
}
