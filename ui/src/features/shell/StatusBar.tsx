// The always-on ops strip along the bottom of the authenticated shell — the one place the session's
// hard facts stay visible while any page is open: WHO you are, WHICH workspace wall you're inside,
// and how much capability the token actually carries (workspace isolation + capability-first, README
// §3). Honest data only: everything here comes from the verified session + the member's resolved
// theme — no polling, no invented "connected" signals. One responsibility: render the strip.

import type { ReactNode } from "react";

import { useTheme, lookById } from "@/lib/theme";

interface Props {
  workspace: string;
  principal: string;
  /** Granted capability count (from the verified token) — the honest "how much can I do" number. */
  capCount: number;
  /** Trailing controls (agent-dock scope: the dock launcher + run pip live here). Rendered at the far
   *  right of the strip. Optional — a shell without a dock (tests) simply omits it. */
  trailing?: ReactNode;
}

export function StatusBar({ workspace, principal, capCount, trailing }: Props) {
  const { theme } = useTheme();
  const look = lookById(theme.look)?.label ?? theme.look;

  return (
    <footer
      aria-label="session status"
      className="relative flex h-7 shrink-0 items-center gap-4 overflow-hidden bg-panel-2/70 px-3 font-mono text-[11px] text-muted"
    >
      {/* The same two-hue hairline the page header carries, mirrored to the top edge of the strip. */}
      <div
        aria-hidden
        className="pointer-events-none absolute inset-x-0 top-0 h-px"
        style={{
          background:
            "linear-gradient(90deg, hsl(var(--accent) / 0.6), hsl(var(--accent-2) / 0.4) 34%, hsl(var(--border)) 72%)",
        }}
      />
      <span className="inline-flex min-w-0 items-center gap-1.5" title={`Workspace ${workspace}`}>
        <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-accent" aria-hidden />
        <span className="truncate text-fg/80">{workspace}</span>
      </span>
      <span className="hidden min-w-0 truncate sm:inline" title={principal}>
        {principal}
      </span>
      <span className="ml-auto hidden shrink-0 md:inline" title="Capabilities granted to this session">
        caps <span className="text-fg/80">{capCount}</span>
      </span>
      <span className="hidden shrink-0 items-center gap-1.5 md:inline-flex" title="Active look">
        <span
          aria-hidden
          className="h-2 w-2 rounded-[3px]"
          style={{ background: "linear-gradient(135deg, hsl(var(--accent)), hsl(var(--accent-2)))" }}
        />
        {look}
      </span>
      {trailing && <span className="shrink-0">{trailing}</span>}
    </footer>
  );
}
