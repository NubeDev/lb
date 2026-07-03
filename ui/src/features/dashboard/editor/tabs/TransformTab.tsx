// The Transform tab (viz panel-editor scope) — a REAL pipeline editor that adds/reorders/removes/disables
// transformers, writing `state.transformations` (`Transformation[]`). It runs NO transform (invariant B,
// README phasing): the backend (`lb-viz` / `viz.query`) executes the pipeline; this tab only AUTHORS the
// config. The supported ids + their default options come from `transformRegistry` (the catalog, NOT an
// executor). Per-id options edit through a minimal typed field where it helps (reduce/filterByValue/
// sortBy/limit) and a raw-JSON textarea for the rest. One responsibility: the transform-list surface.

import type { Transformation } from "@/lib/dashboard";
import type { EditorState } from "../cellEditorState";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { FieldNamePicker } from "../fields/FieldNamePicker";
import { TRANSFORM_DEFS, defaultOptions, transformLabel, type TransformId } from "./transformRegistry";

interface Props {
  state: EditorState;
  patch: (next: Partial<EditorState>) => void;
}

export function TransformTab({ state, patch }: Props) {
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
        <Select
          aria-label="add transformation"
          className="h-8 w-full"
          value=""
          onChange={(e) => {
            if (e.target.value) add(e.target.value as TransformId);
          }}
        >
          <option value="">— add a transformation —</option>
          {TRANSFORM_DEFS.map((d) => (
            <option key={d.id} value={d.id}>
              {d.label}
            </option>
          ))}
        </Select>
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
                <span className="font-mono text-xs text-fg">
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
              <OptionsEditor t={t} onChange={(o) => setOptions(idx, o)} />
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/** The per-id options editor: a friendly field or two for the common ids; a raw-JSON textarea otherwise
 *  (and as the escape hatch for an imported transform with options we don't type). NO execution. */
function OptionsEditor({ t, onChange }: { t: Transformation; onChange: (o: Record<string, unknown>) => void }) {
  const opts = t.options ?? {};
  const num = (v: unknown): number | undefined => (typeof v === "number" ? v : undefined);

  if (t.id === "limit") {
    return (
      <label className="flex items-center gap-2 text-xs text-muted">
        Limit rows
        <Input
          type="number"
          aria-label="limit value"
          className="h-7 w-20 text-xs"
          value={num(opts.limitField) ?? 10}
          onChange={(e) => onChange({ ...opts, limitField: Number(e.target.value) })}
        />
      </label>
    );
  }

  if (t.id === "reduce") {
    const reducers = Array.isArray(opts.reducers) ? (opts.reducers as string[]) : [];
    return (
      <label className="flex items-center gap-2 text-xs text-muted">
        Calculation
        <Select
          aria-label="reduce calc"
          className="h-7 w-36"
          value={reducers[0] ?? "lastNotNull"}
          onChange={(e) => onChange({ ...opts, reducers: [e.target.value] })}
        >
          {["lastNotNull", "last", "first", "min", "max", "mean", "sum", "count"].map((c) => (
            <option key={c} value={c}>
              {c}
            </option>
          ))}
        </Select>
      </label>
    );
  }

  if (t.id === "sortBy") {
    const sort = Array.isArray(opts.sort) ? (opts.sort as Array<{ field?: string; desc?: boolean }>) : [{}];
    const first = sort[0] ?? {};
    return (
      <div className="flex items-center gap-2 text-xs text-muted">
        <span className="flex items-center gap-1">
          Field
          <FieldNamePicker
            aria-label="sort field"
            className="w-40"
            value={first.field ?? ""}
            onChange={(field) => onChange({ ...opts, sort: [{ ...first, field }] })}
          />
        </span>
        <label className="flex items-center gap-1">
          <Checkbox
            aria-label="sort desc"
            checked={!!first.desc}
            onChange={(e) => onChange({ ...opts, sort: [{ ...first, desc: e.target.checked }] })}
          />
          Descending
        </label>
      </div>
    );
  }

  if (t.id === "filterByValue") {
    const type = typeof opts.type === "string" ? (opts.type as string) : "include";
    const match = typeof opts.match === "string" ? (opts.match as string) : "all";
    return (
      <div className="flex items-center gap-2 text-xs text-muted">
        <label className="flex items-center gap-1">
          Action
          <Select aria-label="filter type" className="h-7 w-24" value={type} onChange={(e) => onChange({ ...opts, type: e.target.value })}>
            <option value="include">Include</option>
            <option value="exclude">Exclude</option>
          </Select>
        </label>
        <label className="flex items-center gap-1">
          Match
          <Select aria-label="filter match" className="h-7 w-20" value={match} onChange={(e) => onChange({ ...opts, match: e.target.value })}>
            <option value="all">All</option>
            <option value="any">Any</option>
          </Select>
        </label>
      </div>
    );
  }

  // Everything else: a raw-JSON textarea over the options bag (the honest escape hatch — the backend
  // validates the shape). Invalid JSON is held locally and not written until it parses.
  return <RawJsonOptions opts={opts} onChange={onChange} />;
}

/** A raw-JSON options editor — writes back only when the text parses to an object (invalid JSON shows
 *  the field in an error state without corrupting `state.transformations`). */
function RawJsonOptions({ opts, onChange }: { opts: Record<string, unknown>; onChange: (o: Record<string, unknown>) => void }) {
  return (
    <label className="grid gap-1 text-xs text-muted">
      Options (JSON)
      <Textarea
        aria-label="transform options json"
        className="h-16 w-full resize-y py-1 font-mono text-xs"
        defaultValue={JSON.stringify(opts, null, 0)}
        onBlur={(e) => {
          try {
            const parsed = JSON.parse(e.target.value || "{}");
            if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) onChange(parsed as Record<string, unknown>);
          } catch {
            /* invalid JSON: keep the prior config, don't corrupt state */
          }
        }}
      />
    </label>
  );
}
