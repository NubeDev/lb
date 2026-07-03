// The Transform tab (viz panel-editor scope) — a REAL pipeline editor that adds/reorders/removes/disables
// transformers, writing `state.transformations` (`Transformation[]`). It runs NO transform (invariant B,
// README phasing): the backend (`lb-viz` / `viz.query`) executes the pipeline; this tab only AUTHORS the
// config. Since step 3 (editor-parity) EVERY shipped id has a typed editor (via `TransformEditor`) — the
// raw-JSON textarea survives ONLY for an imported unsupported id. Add-transform is a searchable picker
// with one-line descriptions. One responsibility: the transform-list surface + the add picker.

import { useState } from "react";

import type { Cell, Transformation } from "@/lib/dashboard";
import type { EditorState } from "@/lib/panel-kit/cellEditorState";
import type { VarScope } from "@/lib/vars";
import { Button } from "@/components/ui/button";
import { Combobox } from "@/components/ui/combobox";
import { TRANSFORM_DEFS, defaultOptions, transformLabel, type TransformId } from "./transformRegistry";
import { TransformEditor } from "./transforms/TransformEditor";
import { TransformDebug } from "./transforms/TransformDebug";

interface Props {
  state: EditorState;
  patch: (next: Partial<EditorState>) => void;
  /** The draft cell — needed by the per-step debug view to re-run the pipeline stepwise. */
  draft?: Cell;
  scope?: VarScope;
  refreshKey?: number;
}

export function TransformTab({ state, patch, draft, scope, refreshKey }: Props) {
  const [showSteps, setShowSteps] = useState(false);
  const list = state.transformations;
  const write = (next: Transformation[]) => patch({ transformations: next });

  const add = (id: TransformId) => write([...list, { id, options: defaultOptions(id) }]);
  const remove = (idx: number) => write(list.filter((_, i) => i !== idx));
  const toggle = (idx: number) => write(list.map((t, i) => (i === idx ? { ...t, disabled: !t.disabled } : t)));
  const move = (idx: number, dir: -1 | 1) => {
    const j = idx + dir;
    if (j < 0 || j >= list.length) return;
    const next = [...list];
    [next[idx], next[j]] = [next[j], next[idx]];
    write(next);
  };
  const setOptions = (idx: number, options: Record<string, unknown>) =>
    write(list.map((t, i) => (i === idx ? { ...t, options } : t)));

  return (
    <div className="grid gap-3 py-3" aria-label="transform tab">
      <label className="grid gap-1 text-xs text-muted">
        Add transformation
        <Combobox
          aria-label="add transformation"
          options={TRANSFORM_DEFS.map((d) => ({ value: d.id, label: d.label, description: d.description }))}
          value=""
          placeholder="— add a transformation —"
          onChange={(id) => add(id as TransformId)}
        />
      </label>

      {list.length === 0 ? (
        <p className="text-xs text-muted">
          No transformations. The backend (<code>viz.query</code> / <code>lb-viz</code>) runs the pipeline
          you author here — this tab edits config only, no client-side execution.
        </p>
      ) : (
        <ul className="grid gap-2" aria-label="transform list">
          {list.map((t, idx) => (
            <li key={`${t.id}-${idx}`} className="grid gap-1.5 rounded-md border border-border bg-bg px-2 py-2">
              <div className="flex items-center justify-between gap-2">
                <span className="text-xs font-medium text-fg">
                  {transformLabel(t.id)}
                  {t.disabled ? " (disabled)" : ""}
                </span>
                <div className="flex items-center gap-1">
                  <Button type="button" size="sm" variant="outline" className="h-6 px-1.5 text-xs" aria-label={`move up ${idx}`} disabled={idx === 0} onClick={() => move(idx, -1)}>
                    ↑
                  </Button>
                  <Button type="button" size="sm" variant="outline" className="h-6 px-1.5 text-xs" aria-label={`move down ${idx}`} disabled={idx === list.length - 1} onClick={() => move(idx, 1)}>
                    ↓
                  </Button>
                  <Button type="button" size="sm" variant="outline" className="h-6 px-1.5 text-xs" aria-label={`toggle ${idx}`} onClick={() => toggle(idx)}>
                    {t.disabled ? "enable" : "disable"}
                  </Button>
                  <Button type="button" size="sm" variant="outline" className="h-6 px-1.5 text-xs" aria-label={`remove ${idx}`} onClick={() => remove(idx)}>
                    ✕
                  </Button>
                </div>
              </div>
              <TransformEditor t={t} onChange={(o) => setOptions(idx, o)} />
            </li>
          ))}
        </ul>
      )}

      {/* Per-step debug view (editor-parity step 7): re-runs the pipeline stepwise via viz.query and
          shows the input + the frames after each applied step, so an author sees what each transform did. */}
      {list.length > 0 && draft && (
        <div className="grid gap-2">
          <Button
            type="button"
            size="sm"
            variant={showSteps ? "default" : "outline"}
            aria-label="toggle transform steps"
            aria-pressed={showSteps}
            className="h-7 justify-self-start px-2 text-xs"
            onClick={() => setShowSteps((v) => !v)}
          >
            {showSteps ? "Hide" : "Show"} per-step result
          </Button>
          {showSteps && <TransformDebug draft={draft} scope={scope} refreshKey={refreshKey} />}
        </div>
      )}
    </div>
  );
}
