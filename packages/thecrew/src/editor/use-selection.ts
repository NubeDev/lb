// The keyboard + selection surface: Esc clear/cancel, Del delete, arrows nudge,
// ⌘D duplicate. Click select / shift-add live in ShapeNode.tsx already.
//
// Keyboard split (see use-drag-place.tsx header): selection keys HERE; tool/camera
// keys (V/D/G/Tab/?) in Toolbar.tsx; undo keys in use-undo.ts; chain Enter/Esc in
// use-drag-place.tsx (it also sees Esc — both cancel, which is the intent).
//
// TODO(box select): drag on empty canvas = box select — v1 is click-select only.
// Plan per builder-ux-scope open question: screen-space rect over projected bounds;
// validate perf at ~200 shapes before shipping.

import { useEffect } from "react";
import { useSceneStore } from "../state/scene-store";
import { tokens } from "../theme/tokens";

function inTextInput(e: KeyboardEvent): boolean {
  const t = e.target as HTMLElement | null;
  return !!t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT" || t.isContentEditable);
}

/** Mount once (App). Window-level editor keys for selection gestures. */
export function useEditorKeys(): void {
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (inTextInput(e)) return;
      const s = useSceneStore.getState();

      if ((e.metaKey || e.ctrlKey) && (e.key === "d" || e.key === "D")) {
        e.preventDefault();
        s.duplicateSelection();
        return;
      }
      if (e.metaKey || e.ctrlKey || e.altKey) return;

      const step = tokens.grid.step * (e.shiftKey ? 4 : 1);
      switch (e.key) {
        case "Escape":
          // chain-tool cancel also listens in use-drag-place.tsx — both firing is fine
          s.clearSelection();
          s.armType(null);
          s.setTool("select");
          break;
        case "Delete":
        case "Backspace":
          e.preventDefault();
          s.deleteSelection();
          break;
        case "ArrowUp":
          e.preventDefault();
          s.nudgeSelection(0, step);
          break;
        case "ArrowDown":
          e.preventDefault();
          s.nudgeSelection(0, -step);
          break;
        case "ArrowLeft":
          e.preventDefault();
          s.nudgeSelection(-step, 0);
          break;
        case "ArrowRight":
          e.preventDefault();
          s.nudgeSelection(step, 0);
          break;
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}
