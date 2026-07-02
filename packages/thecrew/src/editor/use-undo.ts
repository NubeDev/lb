// Local undo/redo over the scene store: one completed gesture = one step (place,
// move-end, connect, delete, prop commit), depth ≥50, nothing un-undoable.
// Playground-local by design — the framework wires the core undo journal instead
// (graphics-canvas scope, open question 3).

export function useUndo(): void {
  // TODO(phase 2)
}
