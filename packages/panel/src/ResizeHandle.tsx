// ResizeHandle — the draggable left edge of a right-docked Panel. A thin, hover-lit
// grabber (col-resize cursor) wired to useResizable's handleProps. Keyboard-operable
// (a focusable separator with arrow-key resize) for accessibility.
//
// One responsibility: the edge affordance. The width math lives in useResizable.

import type { Resizable } from "./useResizable";
import { cn } from "./lib/cn";

export interface ResizeHandleProps {
  resizable: Resizable;
  className?: string;
  "aria-label"?: string;
}

/** The Panel's left-edge drag-to-resize grabber. */
export function ResizeHandle({ resizable, className, "aria-label": ariaLabel = "resize panel" }: ResizeHandleProps) {
  return (
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label={ariaLabel}
      tabIndex={0}
      {...resizable.handleProps}
      className={cn(
        "group absolute left-0 top-0 z-10 h-full w-1.5 -translate-x-1/2 cursor-col-resize touch-none select-none",
        "outline-none",
        className,
      )}
    >
      {/* The visible hairline — brightens on hover, drag, and keyboard focus. */}
      <div
        className={cn(
          "mx-auto h-full w-px bg-lbp-border transition-colors",
          "group-hover:w-0.5 group-hover:bg-lbp-accent group-focus-visible:w-0.5 group-focus-visible:bg-lbp-accent",
          resizable.dragging && "w-0.5 bg-lbp-accent",
        )}
      />
    </div>
  );
}
