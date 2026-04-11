import type { CSSProperties, ReactNode } from "react";

export function Row({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <div style={{ padding: "14px 0", borderBottom: "1px solid #2a2b30" }}>
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          gap: 16,
        }}
      >
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ fontSize: 13 }}>{label}</div>
          {hint && (
            <div style={{ fontSize: 11, color: "#8a8c94", marginTop: 2 }}>
              {hint}
            </div>
          )}
        </div>
        <div>{children}</div>
      </div>
    </div>
  );
}

export function Toggle({
  checked,
  disabled,
  onChange,
}: {
  checked: boolean;
  disabled?: boolean;
  onChange: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onChange}
      disabled={disabled}
      aria-pressed={checked}
      style={{
        width: 36,
        height: 20,
        borderRadius: 20,
        border: "1px solid #3a3b42",
        background: checked ? "#4c7dff" : "#2a2b30",
        position: "relative",
        cursor: disabled ? "default" : "pointer",
        padding: 0,
      }}
    >
      <span
        style={{
          position: "absolute",
          top: 2,
          left: checked ? 18 : 2,
          width: 14,
          height: 14,
          borderRadius: "50%",
          background: "#eaeaea",
          transition: "left 0.15s",
          display: "block",
        }}
      />
    </button>
  );
}

export function SectionHeader({
  title,
  subtitle,
}: {
  title: string;
  subtitle?: string;
}) {
  return (
    <div style={{ marginBottom: 4 }}>
      <h2 style={{ margin: 0, fontSize: 16 }}>{title}</h2>
      {subtitle && (
        <p style={{ margin: "4px 0 0", fontSize: 12, color: "#8a8c94" }}>
          {subtitle}
        </p>
      )}
    </div>
  );
}

export const controlBtnStyle: CSSProperties = {
  background: "#2a2b30",
  color: "#eaeaea",
  border: "1px solid #3a3b42",
  borderRadius: 4,
  padding: "4px 10px",
  fontSize: 12,
  cursor: "pointer",
};

export const inputStyle: CSSProperties = {
  background: "#1a1b1e",
  color: "#eaeaea",
  border: "1px solid #3a3b42",
  borderRadius: 4,
  padding: "4px 8px",
  fontSize: 12,
};

export function lsGet<T>(key: string, fallback: T): T {
  try {
    const v = window.localStorage.getItem(key);
    return v == null ? fallback : (JSON.parse(v) as T);
  } catch {
    return fallback;
  }
}

export function lsSet<T>(key: string, v: T): void {
  window.localStorage.setItem(key, JSON.stringify(v));
}
