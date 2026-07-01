// Panel — the reusable, right-docked, RESIZABLE side panel. The ce-wiresheet InspectPanel
// drawer rebuilt on shadcn/ui primitives + scoped --lbp-* tokens. This is the panel the
// nav-rail-scope handover asked for: dense, sectioned, and — the point — resizable, so
// widening it reveals more option columns ("so many options on resize").
//
// Structure (all data-driven via props, ce-specifics dropped):
//   ┌─ ResizeHandle (drag the left edge to widen) ────────────────────────────┐
//   │ header:  title / description  ............................  headerAside  │
//   ├──────────────────────────────────────────────────────────────────────────┤
//   │ body:    scrollable — the host stacks <Section>/<PropTable>/<KV> here     │
//   ├──────────────────────────────────────────────────────────────────────────┤
//   │ footer:  actions (Cancel / Save …)                                        │
//   └──────────────────────────────────────────────────────────────────────────┘
//
// One responsibility: the panel shell + its resize + open/close. Sections and rows are
// the host's job (composed in). Overlay/focus-trap/escape come from the vendored sheet.

import type { ReactNode } from "react";

import { Sheet, SheetContent, SheetTitle, SheetDescription } from "./primitives/sheet";
import { ResizeHandle } from "./ResizeHandle";
import { useResizable } from "./useResizable";
import { cn } from "./lib/cn";

export interface PanelProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** The panel heading. */
  title: ReactNode;
  /** Sub-heading under the title (optional). */
  description?: ReactNode;
  /** Trailing controls on the header row (e.g. a status chip). */
  headerAside?: ReactNode;
  /** The action row pinned to the bottom (e.g. Cancel / Save). Omit for a footer-less panel. */
  footer?: ReactNode;
  /** aria-label on the dialog content. */
  "aria-label"?: string;
  /** Initial width in px. Default 720 — roomy, unlike the old cramped `sm:max-w-3xl`. */
  initialWidth?: number;
  /** Min width in px (default 360). */
  minWidth?: number;
  /** Max width in px (default 1200 — wide enough to reveal every option column). */
  maxWidth?: number;
  /** Extra classes on the docked surface. */
  className?: string;
  /** The scrollable body — the host stacks <Section>/<PropTable>/<KV> here. */
  children: ReactNode;
}

/** The reusable resizable side panel — ce InspectPanel look on shadcn primitives. */
export function Panel({
  open,
  onOpenChange,
  title,
  description,
  headerAside,
  footer,
  "aria-label": ariaLabel,
  initialWidth = 720,
  minWidth = 360,
  maxWidth = 1200,
  className,
  children,
}: PanelProps) {
  const resizable = useResizable({ initial: initialWidth, min: minWidth, max: maxWidth });

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent
        aria-label={ariaLabel}
        style={{ width: resizable.width }}
        className={cn(resizable.dragging && "select-none", className)}
      >
        <ResizeHandle resizable={resizable} />

        <header className="flex items-start justify-between gap-3 border-b border-lbp-border bg-lbp-secondary px-4 py-3">
          <div className="min-w-0">
            <SheetTitle>{title}</SheetTitle>
            {description ? <SheetDescription className="mt-0.5">{description}</SheetDescription> : null}
          </div>
          {headerAside ? <div className="shrink-0">{headerAside}</div> : null}
        </header>

        <div className="min-h-0 flex-1 overflow-auto">{children}</div>

        {footer ? (
          <footer className="flex items-center justify-end gap-2 border-t border-lbp-border bg-lbp-secondary px-4 py-3">
            {footer}
          </footer>
        ) : null}
      </SheetContent>
    </Sheet>
  );
}
