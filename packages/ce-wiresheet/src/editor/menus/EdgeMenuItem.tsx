import { useState } from "react";

// Shared context-menu row. Used by the edge, pane, and node context menus.
// `danger` tints it as a destructive action (delete).
export function EdgeMenuItem({
  label,
  onClick,
  danger,
}: {
  label: string;
  onClick: () => void;
  danger?: boolean;
}) {
  const [hover, setHover] = useState(false);
  return (
    <button
      onClick={onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        width: "100%",
        textAlign: "left",
        background: hover ? (danger ? "hsl(var(--crit) / 0.18)" : "hsl(var(--secondary))") : "transparent",
        color: danger ? "hsl(var(--crit))" : "hsl(var(--foreground))",
        border: "none",
        padding: "6px 10px",
        cursor: "pointer",
        fontFamily: "inherit",
        fontSize: 12,
        borderRadius: 3,
      }}
    >
      {label}
    </button>
  );
}
