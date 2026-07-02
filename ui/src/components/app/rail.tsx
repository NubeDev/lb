// AppRail — the canonical roster aside used by the full-screen surfaces (Flows, Dashboard, Rules):
// a fixed-width, card-toned column with a bordered header control row (label + a "New" action, or a
// create field) over a scrollable body. Extracted so every rail shares identical chrome — same
// width, border, tone, and header height — and can't drift. Sits INSIDE `AppPage`'s body row, below
// the full-width header (see `page.tsx`). One component per file (FILE-LAYOUT).

import type { ReactNode } from "react";

import { cn } from "@/lib/utils";

interface AppRailProps {
  /** aria-label for the aside, e.g. "flow rail". */
  label?: string;
  /** The header control row — a label + "New" button, or an inline create field. Omit for none. */
  header?: ReactNode;
  /** The scrollable roster body (typically a `<ul>` + an empty-state row). */
  children: ReactNode;
  className?: string;
}

export function AppRail({ label, header, children, className }: AppRailProps) {
  return (
    <aside
      aria-label={label}
      className={cn(
        "flex w-64 shrink-0 flex-col border-r border-border bg-panel shadow-sm shadow-black/5",
        className,
      )}
    >
      {header != null && (
        <div className="flex min-h-[3rem] items-center gap-2 border-b border-border px-3 py-2.5">
          {header}
        </div>
      )}
      <div className="min-h-0 flex-1 overflow-auto p-2">{children}</div>
    </aside>
  );
}
