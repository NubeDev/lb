import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { wiresheetPortalRoot } from "../lib/portal";
import { DiagPanel } from "./DiagPanel";
import { EventsPanel } from "./EventsPanel";

// Bottom peek-drawer holding the diagnostics panel and the wire-event log as
// two tabs. Slides up from behind the bottom bar; the ConnectionStatus handle
// in the bar toggles `open`. Both panels stay mounted (display-toggled) while
// the drawer is open so switching tabs preserves their rAF loops and refs;
// closing the drawer unmounts the whole thing (and stops those loops).

const TAB_KEY = "ce-ui.diagdrawer.tab";

export function DiagDrawer({
  open,
  onClose,
  bottomOffset,
  wsRef,
  autoRate,
  manualRate,
  onSetManualRate,
  onToggleAutoRate,
}: {
  open: boolean;
  onClose: () => void;
  /** Height of the bottom bar, so the drawer sits exactly on top of it. */
  bottomOffset: number;
  wsRef: { setRate(hz: number): void; getRate(): number | null } | null;
  autoRate: boolean;
  manualRate: number;
  onSetManualRate: (hz: number) => void;
  onToggleAutoRate: () => void;
}) {
  const [tab, setTab] = useState<"diag" | "events">(() => {
    try {
      return window.localStorage.getItem(TAB_KEY) === "events" ? "events" : "diag";
    } catch {
      return "diag";
    }
  });
  useEffect(() => {
    try {
      window.localStorage.setItem(TAB_KEY, tab);
    } catch {
      /* ignore */
    }
  }, [tab]);

  // Slide-up on open. `open` gates mount, so this is an entry transition only.
  const [shown, setShown] = useState(false);
  useEffect(() => {
    if (!open) {
      setShown(false);
      return;
    }
    const id = requestAnimationFrame(() => setShown(true));
    return () => cancelAnimationFrame(id);
  }, [open]);

  if (!open) return null;

  // Portal to <body>: the editor has a transformed ancestor, which would make
  // `position: fixed` resolve against that box (offset from the viewport) and
  // leave a gap above the bottom bar. Portaling restores viewport-relative
  // fixed positioning, matching the context-menu portals elsewhere.
  return createPortal(
    <div
      style={{
        position: "fixed",
        left: 0,
        right: 0,
        bottom: bottomOffset,
        height: "42vh",
        minHeight: 240,
        zIndex: 29,
        background: "hsl(var(--card) / 0.97)",
        borderTop: "1px solid hsl(var(--border))",
        boxShadow: "0 -8px 24px rgba(0,0,0,0.35)",
        display: "flex",
        flexDirection: "column",
        color: "hsl(var(--foreground))",
        fontFamily: "var(--font-mono)",
        transform: shown ? "translateY(0)" : "translateY(100%)",
        transition: "transform 180ms cubic-bezier(0.22,1,0.36,1)",
      }}
    >
      {/* tab strip */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 4,
          padding: "5px 8px",
          borderBottom: "1px solid hsl(var(--border))",
        }}
      >
        <TabBtn active={tab === "diag"} onClick={() => setTab("diag")}>
          diagnostics
        </TabBtn>
        <TabBtn active={tab === "events"} onClick={() => setTab("events")}>
          events
        </TabBtn>
        <span style={{ flex: 1 }} />
        <button
          onClick={onClose}
          title="Close drawer"
          style={{
            background: "transparent",
            border: "none",
            color: "hsl(var(--muted-foreground))",
            cursor: "pointer",
            fontSize: 14,
            lineHeight: 1,
          }}
        >
          ▾
        </button>
      </div>
      {/* body — both panels mounted, hidden via display so tab flips keep state */}
      <div style={{ flex: 1, position: "relative", minHeight: 0 }}>
        <div style={{ position: "absolute", inset: 0, display: tab === "diag" ? "block" : "none" }}>
          <DiagPanel
            embedded
            wsRef={wsRef}
            autoRate={autoRate}
            manualRate={manualRate}
            onSetManualRate={onSetManualRate}
            onToggleAutoRate={onToggleAutoRate}
          />
        </div>
        <div style={{ position: "absolute", inset: 0, display: tab === "events" ? "block" : "none" }}>
          <EventsPanel embedded />
        </div>
      </div>
    </div>,
    wiresheetPortalRoot(),
  );
}

function TabBtn({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      style={{
        fontSize: 11,
        padding: "3px 12px",
        background: active ? "hsl(var(--cool))" : "hsl(var(--secondary))",
        color: active ? "#fff" : "hsl(var(--foreground))",
        border: "1px solid hsl(var(--border))",
        borderRadius: 4,
        cursor: "pointer",
        fontFamily: "inherit",
      }}
    >
      {children}
    </button>
  );
}
