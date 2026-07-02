// KV — the ce InspectPanel key/value row, ported to shadcn tokens. A fixed-width muted
// key beside its value, monospace, tight vertical rhythm. Used for the Metadata /
// Presentation style sections where a table would be overkill.
//
// One responsibility: one key/value row. Stack several inside a <Section>.

import type { ReactNode } from "react";
import { cn } from "./lib/cn";

export interface KVProps {
  k: ReactNode;
  v: ReactNode;
  /** Key-column width in px (ce uses 80). */
  keyWidth?: number;
  className?: string;
}

/** A dense key/value row — the ce InspectPanel `KV` look on shadcn tokens. */
export function KV({ k, v, keyWidth = 80, className }: KVProps) {
  return (
    <div className={cn("flex gap-2 py-[2px] font-mono text-[11px]", className)}>
      <span style={{ width: keyWidth }} className="shrink-0 text-lbp-muted">
        {k}
      </span>
      <span className="min-w-0 break-words text-lbp-fg">{v}</span>
    </div>
  );
}
