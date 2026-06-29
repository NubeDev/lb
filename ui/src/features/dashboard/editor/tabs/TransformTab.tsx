// The Transform tab (viz panel-editor scope: ship the full tab structure from day one). Per invariant
// B (README phasing), Phase 1 builds NO client-side transform library — the pipeline is born in the
// backend (`lb-viz` + `viz.query`) in Phase 3. So this tab is a deliberate SHELL: it states the phasing
// honestly and edits only `transformations[]` CONFIG on the cell (none addable yet). It exists now so
// adding the Phase-3 transforms never reintroduces an add≠edit fork. One responsibility: the (empty)
// transform list surface.

import type { EditorState } from "../cellEditorState";

interface Props {
  state: EditorState;
  patch: (next: Partial<EditorState>) => void;
}

export function TransformTab({ state }: Props) {
  return (
    <div className="grid gap-2 py-3 text-xs text-muted" aria-label="transform tab">
      {state.transformations.length === 0 ? (
        <p>
          No transformations. The transformation pipeline runs in the backend (<code>viz.query</code> /{" "}
          <code>lb-viz</code>) and arrives in a later phase — there is no client-side transform here by
          design. Saved transformation config round-trips on the cell.
        </p>
      ) : (
        <ul className="grid gap-1" aria-label="transform list">
          {state.transformations.map((t, idx) => (
            <li key={`${t.id}-${idx}`} className="rounded-md border border-border bg-bg px-2 py-1 font-mono">
              {t.id}
              {t.disabled ? " (disabled)" : ""}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
