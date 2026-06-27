// Shared widget chrome — the tiny header + the loading/empty/denied state every built-in widget
// shows (dashboard scope). A bound series the viewer isn't granted renders an honest `denied` state,
// never a fake value (the no-mock rule applies to the UI). One owner of these so they don't drift.

import type { ReactNode } from "react";

/** The truncated series/label header at the top of a widget. */
export function WidgetHeader({ label }: { label: string }) {
  return <div className="truncate text-xs font-medium text-muted">{label}</div>;
}

/** A centered status message — `denied` (no access) is red, everything else muted. */
export function WidgetMessage({
  tone,
  children,
}: {
  tone: "muted" | "denied";
  children: ReactNode;
}) {
  const cls = tone === "denied" ? "text-red-400" : "text-muted";
  return (
    <div className={`flex h-full items-center justify-center text-xs ${cls}`} role="status">
      {children}
    </div>
  );
}
