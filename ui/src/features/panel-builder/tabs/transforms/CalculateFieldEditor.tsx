// The calculateField transform editor (editor-parity scope, step 3) — mode + operand pickers. Backend
// modes verbatim (`rust/crates/viz` calculate_field): binary ({left,operator,right} where each side is
// {field} or {fixed}), reduceRow ({reducer, include[]}), index (row number), unary ({operator,
// fieldName}). Plus `alias` + `replaceFields`. Each side's field is a FieldNamePicker over real fields.
// One responsibility: author a calculateField config.

import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Checkbox } from "@/components/ui/checkbox";
import { FieldNamePicker } from "../../fields/FieldNamePicker";

interface Props {
  options: Record<string, unknown>;
  onChange: (options: Record<string, unknown>) => void;
}

type Side = { field?: string; fixed?: number };
const OPS = ["+", "-", "*", "/"];
const UNARY_OPS = ["abs", "exp", "ln", "log2", "log10", "floor", "ceil"];
const REDUCERS = ["sum", "mean", "min", "max", "count", "last"];

/** A picker for one binary operand: a field OR a fixed number (radio-ish via a small mode select). */
function SideEditor({ label, side, onChange }: { label: string; side: Side; onChange: (s: Side) => void }) {
  const mode = side.fixed !== undefined ? "fixed" : "field";
  return (
    <span className="flex items-center gap-1">
      <Select aria-label={`${label} mode`} className="h-7 w-20" value={mode} onChange={(e) => onChange(e.target.value === "fixed" ? { fixed: 0 } : { field: "" })}>
        <option value="field">Field</option>
        <option value="fixed">Number</option>
      </Select>
      {mode === "field" ? (
        <FieldNamePicker aria-label={`${label} field`} className="w-28" value={side.field ?? ""} onChange={(field) => onChange({ field })} />
      ) : (
        <Input aria-label={`${label} fixed`} type="number" className="h-7 w-20 text-xs" value={side.fixed ?? 0} onChange={(e) => onChange({ fixed: Number(e.target.value) })} />
      )}
    </span>
  );
}

export function CalculateFieldEditor({ options, onChange }: Props) {
  const mode = typeof options.mode === "string" ? (options.mode as string) : "binary";
  const alias = typeof options.alias === "string" ? (options.alias as string) : "";
  const replaceFields = options.replaceFields === true;
  const set = (patch: Record<string, unknown>) => onChange({ ...options, ...patch });

  const binary = (options.binary && typeof options.binary === "object" ? options.binary : {}) as {
    left?: Side;
    operator?: string;
    right?: Side;
  };
  const reduce = (options.reduce && typeof options.reduce === "object" ? options.reduce : {}) as {
    reducer?: string;
  };
  const unary = (options.unary && typeof options.unary === "object" ? options.unary : {}) as {
    operator?: string;
    fieldName?: string;
  };

  return (
    <div className="grid gap-2 text-xs text-muted" aria-label="calculate field editor">
      <label className="flex items-center gap-1">
        Mode
        <Select aria-label="calc mode" className="h-7 w-40" value={mode} onChange={(e) => set({ mode: e.target.value })}>
          <option value="binary">Binary operation</option>
          <option value="reduceRow">Reduce row</option>
          <option value="unary">Unary operation</option>
          <option value="index">Row index</option>
        </Select>
      </label>

      {mode === "binary" && (
        <div className="flex flex-wrap items-center gap-1.5">
          <SideEditor label="left operand" side={binary.left ?? { field: "" }} onChange={(left) => set({ binary: { ...binary, left } })} />
          <Select aria-label="calc operator" className="h-7 w-16" value={binary.operator ?? "+"} onChange={(e) => set({ binary: { ...binary, operator: e.target.value } })}>
            {OPS.map((o) => (
              <option key={o} value={o}>
                {o}
              </option>
            ))}
          </Select>
          <SideEditor label="right operand" side={binary.right ?? { field: "" }} onChange={(right) => set({ binary: { ...binary, right } })} />
        </div>
      )}

      {mode === "reduceRow" && (
        <label className="flex items-center gap-1">
          Calculation
          <Select aria-label="calc reducer" className="h-7 w-32" value={reduce.reducer ?? "sum"} onChange={(e) => set({ reduce: { ...reduce, reducer: e.target.value } })}>
            {REDUCERS.map((r) => (
              <option key={r} value={r}>
                {r}
              </option>
            ))}
          </Select>
        </label>
      )}

      {mode === "unary" && (
        <div className="flex items-center gap-1.5">
          <Select aria-label="calc unary op" className="h-7 w-24" value={unary.operator ?? "abs"} onChange={(e) => set({ unary: { ...unary, operator: e.target.value } })}>
            {UNARY_OPS.map((o) => (
              <option key={o} value={o}>
                {o}
              </option>
            ))}
          </Select>
          <FieldNamePicker aria-label="calc unary field" className="w-28" value={unary.fieldName ?? ""} onChange={(fieldName) => set({ unary: { ...unary, fieldName } })} />
        </div>
      )}

      <label className="flex items-center gap-1">
        Alias
        <Input aria-label="calc alias" className="h-7 flex-1 text-xs" placeholder="new field name" value={alias} onChange={(e) => set({ alias: e.target.value })} />
      </label>
      <label className="flex items-center gap-2">
        <Checkbox aria-label="calc replace fields" checked={replaceFields} onChange={(e) => set({ replaceFields: e.target.checked })} />
        Replace all fields
      </label>
    </div>
  );
}
