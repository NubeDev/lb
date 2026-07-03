// The small single-form transform editors (editor-parity scope, step 3) — the ids whose config is one
// or two typed fields: reduce (calculation), sortBy (field + desc), limit (row count),
// filterFieldsByName (include/exclude by names — a multi field picker + a pattern), joinByField
// (byField + inner/outer). merge/seriesToRows have no options (a note). Kept together (each is <30
// lines; splitting to 6 more files would be noise) — one responsibility PER exported editor. Backend
// shapes verbatim (`rust/crates/viz`).

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Checkbox } from "@/components/ui/checkbox";
import { FieldNamePicker } from "../../fields/FieldNamePicker";

interface EditorProps {
  options: Record<string, unknown>;
  onChange: (options: Record<string, unknown>) => void;
}

const num = (v: unknown): number | undefined => (typeof v === "number" ? v : undefined);

export function ReduceEditor({ options, onChange }: EditorProps) {
  const reducers = Array.isArray(options.reducers) ? (options.reducers as string[]) : [];
  return (
    <label className="flex items-center gap-2 text-xs text-muted">
      Calculation
      <Select aria-label="reduce calc" className="h-7 w-36" value={reducers[0] ?? "lastNotNull"} onChange={(e) => onChange({ ...options, reducers: [e.target.value] })}>
        {["lastNotNull", "last", "first", "min", "max", "mean", "sum", "count"].map((c) => (
          <option key={c} value={c}>
            {c}
          </option>
        ))}
      </Select>
    </label>
  );
}

export function SortByEditor({ options, onChange }: EditorProps) {
  const sort = Array.isArray(options.sort) ? (options.sort as Array<{ field?: string; desc?: boolean }>) : [{}];
  const first = sort[0] ?? {};
  return (
    <div className="flex items-center gap-2 text-xs text-muted">
      <span className="flex items-center gap-1">
        Field
        <FieldNamePicker aria-label="sort field" className="w-40" value={first.field ?? ""} onChange={(field) => onChange({ ...options, sort: [{ ...first, field }] })} />
      </span>
      <label className="flex items-center gap-1">
        <Checkbox aria-label="sort desc" checked={!!first.desc} onChange={(e) => onChange({ ...options, sort: [{ ...first, desc: e.target.checked }] })} />
        Descending
      </label>
    </div>
  );
}

export function LimitEditor({ options, onChange }: EditorProps) {
  return (
    <label className="flex items-center gap-2 text-xs text-muted">
      Limit rows
      <Input type="number" aria-label="limit value" className="h-7 w-20 text-xs" value={num(options.limitField) ?? 10} onChange={(e) => onChange({ ...options, limitField: Number(e.target.value) })} />
    </label>
  );
}

/** filterFieldsByName — include by an explicit name set (chips) + an optional regex pattern. Exclude is
 *  the same shape; Grafana's common case is include, which we surface here (exclude stays import-safe). */
export function FilterFieldsByNameEditor({ options, onChange }: EditorProps) {
  const include = (options.include && typeof options.include === "object" ? options.include : {}) as {
    names?: string[];
    pattern?: string;
  };
  const names = include.names ?? [];
  const setInclude = (patch: Partial<typeof include>) => onChange({ ...options, include: { ...include, ...patch } });
  const addName = (name: string) => {
    if (name && !names.includes(name)) setInclude({ names: [...names, name] });
  };
  const removeName = (name: string) => setInclude({ names: names.filter((n) => n !== name) });
  return (
    <div className="grid gap-1.5 text-xs text-muted" aria-label="filter fields editor">
      <span className="flex items-center gap-1">
        Add field
        <FieldNamePicker aria-label="filter fields add" className="w-40" value="" onChange={addName} />
      </span>
      {names.length > 0 && (
        <div className="flex flex-wrap gap-1" aria-label="filter fields chips">
          {names.map((n) => (
            <Button key={n} type="button" size="sm" variant="outline" aria-label={`remove field ${n}`} className="h-auto px-2 py-0.5 text-[11px]" onClick={() => removeName(n)}>
              {n} ×
            </Button>
          ))}
        </div>
      )}
      <label className="flex items-center gap-1">
        Regex
        <Input aria-label="filter fields pattern" className="h-7 flex-1 text-xs" placeholder="optional pattern" value={include.pattern ?? ""} onChange={(e) => setInclude({ pattern: e.target.value || undefined })} />
      </label>
    </div>
  );
}

export function JoinByFieldEditor({ options, onChange }: EditorProps) {
  const byField = typeof options.byField === "string" ? (options.byField as string) : "";
  const mode = typeof options.mode === "string" ? (options.mode as string) : "outer";
  return (
    <div className="flex items-center gap-2 text-xs text-muted">
      <span className="flex items-center gap-1">
        Field
        <FieldNamePicker aria-label="join field" className="w-36" value={byField} onChange={(v) => onChange({ ...options, byField: v })} />
      </span>
      <label className="flex items-center gap-1">
        Mode
        <Select aria-label="join mode" className="h-7 w-24" value={mode} onChange={(e) => onChange({ ...options, mode: e.target.value })}>
          <option value="outer">Outer</option>
          <option value="inner">Inner</option>
        </Select>
      </label>
    </div>
  );
}

export function NoOptionsEditor({ id }: { id: string }) {
  return <p className="text-xs text-muted">{id} has no options.</p>;
}
