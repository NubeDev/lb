// A draggable horizontal divider for a two-pane vertical stack: a TOP pane over a BOTTOM pane. Owns
// the split ratio (top fraction of the container height) and the pointer-drag that adjusts it,
// clamped so neither pane collapses. View-agnostic: it returns the top pane's flex-basis and the
// divider's drag handler; the caller renders the panes and the handle (see `<SplitHandle/>`).
// Shared by the Data Studio panel builder (preview / options) and the Rules editor (code / result).

import { useCallback, useRef, useState } from "react";

/** Minimum fraction of the container each pane keeps, so a drag can't collapse either side. */
const MIN_FRACTION = 0.15;

export interface VerticalSplit {
  /** `ref` for the split container — the drag maps pointer Y against its measured box. */
  containerRef: React.RefObject<HTMLDivElement>;
  /** flex-basis (`%`) for the TOP pane; the bottom pane takes the remainder via `flex-1`. */
  topBasis: string;
  /** onPointerDown for the divider handle. */
  onHandleDown: (e: React.PointerEvent) => void;
  /** True while dragging — the caller can disable pane pointer-events to keep the drag smooth. */
  dragging: boolean;
}

export function useVerticalSplit(initialTopFraction = 0.6): VerticalSplit {
  const containerRef = useRef<HTMLDivElement>(null);
  const [fraction, setFraction] = useState(initialTopFraction);
  const [dragging, setDragging] = useState(false);

  const onHandleDown = useCallback((e: React.PointerEvent) => {
    e.preventDefault();
    const el = containerRef.current;
    if (!el) return;
    setDragging(true);
    (e.target as Element).setPointerCapture?.(e.pointerId);

    const onMove = (ev: PointerEvent) => {
      const box = el.getBoundingClientRect();
      if (box.height <= 0) return;
      const f = (ev.clientY - box.top) / box.height;
      setFraction(Math.min(1 - MIN_FRACTION, Math.max(MIN_FRACTION, f)));
    };
    const onUp = () => {
      setDragging(false);
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  }, []);

  return { containerRef, topBasis: `${(fraction * 100).toFixed(2)}%`, onHandleDown, dragging };
}
