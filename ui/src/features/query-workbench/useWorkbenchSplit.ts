// The editor/results vertical split for the Query workbench (datasources-ux scope). The workbench
// stacks: editor → run bar → divider → results. This hook owns the editor's share of the vertical
// space, the drag-to-resize gesture on the divider, and the maximise/restore toggles. One
// responsibility per file (FILE-LAYOUT): the split geometry only — the workbench semantics stay in
// QueryWorkbench.tsx.
//
// The ratio is the editor's fraction of the available height (0–1). Dragging the divider adjusts it;
// each section keeps a floor (`MIN_RATIO`) so neither fully collapses. Maximise pushes one section to
// `MAX_RATIO` and leaves the other a sliver; clicking again restores the default 50/50 split. The
// editor section carries `overflow-y-auto` (applied by the caller) so a tall Builder never gets
// clipped — the ratio bounds its height and the content scrolls within it.

import { useCallback, useEffect, useRef, useState } from "react";

const MIN_RATIO = 0.12;
const DEFAULT_RATIO = 0.5;
const MAX_RATIO = 0.88;

export interface WorkbenchSplit {
  /** Inline style for the editor section — applies the flex share + `minHeight: 0` (so it can scroll). */
  editorStyle: React.CSSProperties;
  /** Inline style for the results section. */
  resultsStyle: React.CSSProperties;
  /** Spread on the divider element — wires up the drag-to-resize gesture + ARIA separator semantics. */
  dividerProps: {
    onMouseDown: (e: React.MouseEvent) => void;
    role: "separator";
    "aria-orientation": "horizontal";
    "aria-label": string;
    "aria-valuenow": number;
    tabIndex: 0;
  };
  /** Toggle the editor between maximised and the default split. */
  toggleEditor: () => void;
  /** Toggle the results between maximised and the default split. */
  toggleResults: () => void;
  editorMaximised: boolean;
  resultsMaximised: boolean;
}

function clampRatio(r: number): number {
  return Math.max(MIN_RATIO, Math.min(1 - MIN_RATIO, r));
}

/** Manage the editor/results split ratio, the drag gesture, and the maximise/restore toggles. */
export function useWorkbenchSplit(containerRef: React.RefObject<HTMLElement | null>): WorkbenchSplit {
  const [ratio, setRatio] = useState(DEFAULT_RATIO);
  const drag = useRef<{ startY: number; startRatio: number; h: number } | null>(null);

  const onMove = useCallback((e: MouseEvent) => {
    if (!drag.current) return;
    const delta = (e.clientY - drag.current.startY) / drag.current.h;
    setRatio(clampRatio(drag.current.startRatio + delta));
  }, []);

  const stop = useCallback(() => {
    if (!drag.current) return;
    drag.current = null;
    document.body.style.userSelect = "";
    document.body.style.cursor = "";
    document.removeEventListener("mousemove", onMove);
    document.removeEventListener("mouseup", stop);
  }, [onMove]);

  const onMouseDown = useCallback(
    (e: React.MouseEvent) => {
      const container = containerRef.current;
      if (!container) return;
      e.preventDefault();
      const h = container.getBoundingClientRect().height;
      if (h <= 0) return;
      drag.current = { startY: e.clientY, startRatio: ratio, h };
      document.body.style.userSelect = "none";
      document.body.style.cursor = "row-resize";
      document.addEventListener("mousemove", onMove);
      document.addEventListener("mouseup", stop);
    },
    [containerRef, ratio, onMove, stop],
  );

  // Release the gesture + restore styles if the component unmounts mid-drag.
  useEffect(() => () => stop(), [stop]);

  const editorMaximised = ratio >= MAX_RATIO - 0.01;
  const resultsMaximised = ratio <= MIN_RATIO + 0.01;

  return {
    editorStyle: { flex: `${ratio} 1 0%`, minHeight: 0 },
    resultsStyle: { flex: `${1 - ratio} 1 0%`, minHeight: 0 },
    dividerProps: {
      onMouseDown,
      role: "separator",
      "aria-orientation": "horizontal",
      "aria-label": "resize editor and results",
      "aria-valuenow": Math.round(ratio * 100),
      tabIndex: 0,
    },
    toggleEditor: () => setRatio(editorMaximised ? DEFAULT_RATIO : MAX_RATIO),
    toggleResults: () => setRatio(resultsMaximised ? DEFAULT_RATIO : MIN_RATIO),
    editorMaximised,
    resultsMaximised,
  };
}
