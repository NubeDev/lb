// Undo/redo KEYBINDINGS only — the undo stack itself lives in scene-store.ts
// (snapshot per completed gesture, depth ≥50, nothing un-undoable). This hook just
// maps ⌘/Ctrl-Z → undo, ⇧⌘Z / Ctrl-Y → redo. Playground-local by design — the
// framework wires the core undo journal instead (graphics-canvas scope, open q 3).
//
// Keyboard split: undo keys HERE; selection keys in use-selection.ts; tool/camera
// keys in Toolbar.tsx; chain-tool keys in use-drag-place.tsx.

import { useEffect } from "react";
import { useSceneStore } from "../state/scene-store";

/** Mount once (App). Window-level undo/redo keys. */
export function useUndoKeys(): void {
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      const t = e.target as HTMLElement | null;
      if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT" || t.isContentEditable))
        return;
      if (!(e.metaKey || e.ctrlKey)) return;
      const s = useSceneStore.getState();
      if (e.key === "z" || e.key === "Z") {
        e.preventDefault();
        if (e.shiftKey) s.redo();
        else s.undo();
      } else if (e.key === "y" || e.key === "Y") {
        e.preventDefault();
        s.redo();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}
