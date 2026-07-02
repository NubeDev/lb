import { useState } from "react";

// Floating error toast pinned to the bottom-center of the canvas. Carries an
// optional `debug` payload (raw request+response) that the copy button prefers
// over the human-facing message.
export function ErrorBanner({
  error,
  onClose,
}: {
  error: { message: string; debug?: string };
  onClose: () => void;
}) {
  const [copied, setCopied] = useState(false);
  const copy = (e: React.MouseEvent) => {
    e.stopPropagation();
    const text = error.debug ?? error.message;
    void navigator.clipboard?.writeText(text).then(
      () => {
        setCopied(true);
        window.setTimeout(() => setCopied(false), 1200);
      },
      () => {},
    );
  };
  return (
    <div
      style={{
        position: "fixed",
        bottom: 12,
        left: "50%",
        transform: "translateX(-50%)",
        zIndex: 30,
        maxWidth: "min(720px, 90vw)",
        background: "hsl(var(--crit) / 0.18)",
        border: "1px solid hsl(var(--crit) / 0.25)",
        color: "hsl(var(--crit))",
        padding: "6px 10px",
        borderRadius: 4,
        fontSize: 12,
        fontFamily: "var(--font-mono)",
        display: "flex",
        alignItems: "flex-start",
        gap: 8,
      }}
    >
      <span style={{ whiteSpace: "pre-wrap", overflow: "hidden", flex: 1, maxHeight: 120 }}>
        {error.message}
      </span>
      <button
        onClick={copy}
        title={error.debug ? "Copy request + response" : "Copy error"}
        style={{
          flexShrink: 0,
          background: "hsl(var(--crit) / 0.3)",
          color: "hsl(var(--crit))",
          border: "1px solid hsl(var(--crit) / 0.25)",
          borderRadius: 3,
          padding: "1px 8px",
          fontSize: 11,
          cursor: "pointer",
          fontFamily: "inherit",
        }}
      >
        {copied ? "copied" : "copy"}
      </button>
      <button
        onClick={onClose}
        title="Dismiss"
        style={{
          flexShrink: 0,
          background: "transparent",
          color: "hsl(var(--crit))",
          border: "none",
          fontSize: 13,
          cursor: "pointer",
          lineHeight: 1,
          padding: "0 2px",
        }}
      >
        ✕
      </button>
    </div>
  );
}
