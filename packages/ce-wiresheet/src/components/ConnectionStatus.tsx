import { useEffect, useRef } from "react";
import { metrics } from "../lib/instrumentation";

// Bottom-bar connection indicator + handle for the diagnostics/events drawer.
// The dot + label are driven by a self-contained rAF reading `metrics.wsConnected`
// (set directly by the WS layer), so it stays live regardless of whether the
// drawer — and the DiagPanel inside it — is mounted.
export function ConnectionStatus({
  open,
  onToggle,
}: {
  open: boolean;
  onToggle: () => void;
}) {
  const rDot = useRef<HTMLSpanElement>(null);
  const rTxt = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    let raf = 0;
    let last: boolean | null = null;
    const tick = () => {
      raf = requestAnimationFrame(tick);
      if (last === metrics.wsConnected) return;
      last = metrics.wsConnected;
      if (rDot.current) rDot.current.style.background = last ? "hsl(var(--green))" : "hsl(var(--crit))";
      if (rTxt.current) rTxt.current.textContent = last ? "connected" : "disconnected";
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, []);
  return (
    <button
      onClick={onToggle}
      title={open ? "Hide diagnostics & events" : "Show diagnostics & events"}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 7,
        padding: "3px 10px",
        background: open ? "hsl(var(--secondary))" : "transparent",
        border: "1px solid hsl(var(--border))",
        borderRadius: 5,
        color: "hsl(var(--foreground))",
        fontSize: 11,
        fontFamily: "var(--font-mono)",
        cursor: "pointer",
      }}
    >
      <span
        ref={rDot}
        style={{ width: 8, height: 8, borderRadius: 4, background: "hsl(var(--crit))", display: "inline-block" }}
      />
      <span ref={rTxt}>disconnected</span>
      <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 10 }}>{open ? "▾" : "▴"}</span>
    </button>
  );
}
