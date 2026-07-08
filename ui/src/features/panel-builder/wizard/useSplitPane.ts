// useSplitPane (panel-wizard scope) — the draggable divider between the wizard's step column and the
// pinned preview. Pure presentation state: a left-column fraction, clamped so neither pane collapses,
// persisted per-browser (localStorage) so the author's preferred balance survives reloads. Pointer-drag
// AND keyboard (arrow keys on the separator) adjust it — never authoring state, never in the saved cell.
// One responsibility: own the split fraction + the drag/keyboard handlers.

import { useCallback, useRef, useState } from "react";

const KEY = "lb.panel-wizard.split";
const MIN = 0.28;
const MAX = 0.72;
const DEFAULT = 0.48;

/** Clamp into MIN..MAX and round to 3 decimals (stable style strings; no float-noise re-renders). */
const clamp = (f: number) => Math.round(Math.min(MAX, Math.max(MIN, f)) * 1000) / 1000;

export interface SplitPane {
  /** The left column's fraction of the container width (MIN..MAX). */
  fraction: number;
  /** Attach to the split container (the width reference for the drag math). */
  containerRef: React.RefObject<HTMLDivElement>;
  /** The separator's pointer-down — captures the pointer and drags the fraction. */
  onPointerDown: (e: React.PointerEvent<HTMLElement>) => void;
  /** The separator's keydown — ←/→ nudge the fraction (accessible resize). */
  onKeyDown: (e: React.KeyboardEvent<HTMLElement>) => void;
}

export function useSplitPane(): SplitPane {
  const containerRef = useRef<HTMLDivElement>(null!);
  const [fraction, setFraction] = useState<number>(() => {
    const saved = Number(window.localStorage?.getItem(KEY));
    return Number.isFinite(saved) && saved > 0 ? clamp(saved) : DEFAULT;
  });

  const commit = useCallback((f: number) => {
    const next = clamp(f);
    setFraction(next);
    try {
      window.localStorage?.setItem(KEY, String(next));
    } catch {
      // storage full/denied — the in-memory fraction still works for this session
    }
  }, []);

  const onPointerDown = useCallback(
    (e: React.PointerEvent<HTMLElement>) => {
      const box = containerRef.current?.getBoundingClientRect();
      if (!box || box.width === 0) return;
      e.preventDefault();
      const target = e.currentTarget;
      target.setPointerCapture(e.pointerId);
      const move = (ev: PointerEvent) => commit((ev.clientX - box.left) / box.width);
      const up = () => {
        target.removeEventListener("pointermove", move);
        target.removeEventListener("pointerup", up);
      };
      target.addEventListener("pointermove", move);
      target.addEventListener("pointerup", up);
    },
    [commit],
  );

  const onKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLElement>) => {
      if (e.key === "ArrowLeft") commit(fraction - 0.02);
      else if (e.key === "ArrowRight") commit(fraction + 0.02);
      else return;
      e.preventDefault();
    },
    [commit, fraction],
  );

  return { fraction, containerRef, onPointerDown, onKeyDown };
}
