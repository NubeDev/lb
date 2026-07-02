// The zustand store: scene doc + selection + camera mode + undo stack.
// Single writer for the document; editor hooks dispatch through here so every
// completed gesture is one undo step (builder-ux-scope.md §undo).

import type { SceneDoc } from "../scene/scene.types";

export interface SceneStore {
  doc: SceneDoc;
  selection: string[];
  // TODO(phase 2): actions — placeShape, moveShape, setProp, deleteSelection,
  // select/box-select, toggleCamera, undo/redo (use-undo.ts drives the stack).
}

export function useSceneStore(): SceneStore {
  throw new Error("TODO(phase 2): zustand store");
}
