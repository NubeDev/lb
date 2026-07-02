// useResizable — a dependency-free pointer-drag width controller for a right-docked
// panel. The panel is anchored to the right edge; dragging its LEFT edge changes width
// (drag left → wider → more option columns fit; this is the "so many options on resize"
// behavior). Kept as its own hook (FILE-LAYOUT) so the shell stays declarative.
//
// Width is clamped to [min, max] and the drag uses pointer capture so it survives the
// cursor leaving the handle. No global listeners left dangling — capture/release pair on
// the handle element.

import { useCallback, useRef, useState } from "react";

export interface Resizable {
  /** Current width in px. */
  width: number;
  /** Whether a drag is in progress (host can dim/limit repaint). */
  dragging: boolean;
  /** Spread onto the drag handle element. */
  handleProps: {
    onPointerDown: (e: React.PointerEvent) => void;
    onPointerMove: (e: React.PointerEvent) => void;
    onPointerUp: (e: React.PointerEvent) => void;
    onKeyDown: (e: React.KeyboardEvent) => void;
  };
}

export interface UseResizableOptions {
  initial: number;
  min: number;
  max: number;
  /** Arrow-key step for keyboard resize (accessibility). */
  step?: number;
}

/** Controls a right-docked panel's width via a left-edge drag handle. */
export function useResizable({ initial, min, max, step = 24 }: UseResizableOptions): Resizable {
  const clamp = useCallback((w: number) => Math.min(max, Math.max(min, w)), [min, max]);
  const [width, setWidth] = useState(() => clamp(initial));
  const [dragging, setDragging] = useState(false);
  // Anchor captured at drag start: pointer X + width then, so movement is relative.
  const start = useRef<{ x: number; w: number } | null>(null);

  const onPointerDown = useCallback(
    (e: React.PointerEvent) => {
      start.current = { x: e.clientX, w: width };
      setDragging(true);
      e.currentTarget.setPointerCapture(e.pointerId);
      e.preventDefault();
    },
    [width],
  );

  const onPointerMove = useCallback(
    (e: React.PointerEvent) => {
      if (!start.current) return;
      // Right-docked: dragging left (clientX decreases) widens the panel.
      const delta = start.current.x - e.clientX;
      setWidth(clamp(start.current.w + delta));
    },
    [clamp],
  );

  const onPointerUp = useCallback((e: React.PointerEvent) => {
    start.current = null;
    setDragging(false);
    if (e.currentTarget.hasPointerCapture(e.pointerId)) {
      e.currentTarget.releasePointerCapture(e.pointerId);
    }
  }, []);

  const onKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "ArrowLeft") {
        setWidth((w) => clamp(w + step));
        e.preventDefault();
      } else if (e.key === "ArrowRight") {
        setWidth((w) => clamp(w - step));
        e.preventDefault();
      }
    },
    [clamp, step],
  );

  return { width, dragging, handleProps: { onPointerDown, onPointerMove, onPointerUp, onKeyDown } };
}
