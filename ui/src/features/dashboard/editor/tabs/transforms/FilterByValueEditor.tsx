// The filterByValue transform editor (editor-parity scope, step 3) — REAL condition rows (before this,
// only action/match, no conditions!). Grafana/backend shape verbatim (`rust/crates/viz` filter_by_value):
//   { type: include|exclude, match: any|all, filters: [{ fieldName, config: { id, options } }] }
// Each row = a field picker + a matcher operator + its operand(s). Matcher ids are the backend's:
// greater/greaterOrEqual/lower/lowerOrEqual (numeric operand), equal/notEqual (any operand), regex
// (pattern), isNull/isNotNull (no operand). One responsibility: author a filterByValue config.

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { FieldNamePicker } from "../../fields/FieldNamePicker";

interface Props {
  options: Record<string, unknown>;
  onChange: (options: Record<string, unknown>) => void;
}

interface Filter {
  fieldName?: string;
  config?: { id?: string; options?: { value?: unknown; from?: number; to?: number } };
}

const MATCHERS: Array<{ id: string; label: string; operand: "number" | "text" | "none" | "range" }> = [
  { id: "greater", label: ">", operand: "number" },
  { id: "greaterOrEqual", label: "≥", operand: "number" },
  { id: "lower", label: "<", operand: "number" },
  { id: "lowerOrEqual", label: "≤", operand: "number" },
  { id: "equal", label: "=", operand: "text" },
  { id: "notEqual", label: "≠", operand: "text" },
  { id: "regex", label: "matches regex", operand: "text" },
  { id: "isNull", label: "is null", operand: "none" },
  { id: "isNotNull", label: "is not null", operand: "none" },
];

function operandKind(id: string | undefined): "number" | "text" | "none" | "range" {
  return MATCHERS.find((m) => m.id === id)?.operand ?? "text";
}

export function FilterByValueEditor({ options, onChange }: Props) {
  const type = typeof options.type === "string" ? (options.type as string) : "include";
  const match = typeof options.match === "string" ? (options.match as string) : "all";
  const filters: Filter[] = Array.isArray(options.filters) ? (options.filters as Filter[]) : [];

  const write = (next: Filter[]) => onChange({ ...options, type, match, filters: next });
  const setFilter = (idx: number, next: Filter) => write(filters.map((f, i) => (i === idx ? next : f)));
  const remove = (idx: number) => write(filters.filter((_, i) => i !== idx));
  const add = () => write([...filters, { fieldName: "", config: { id: "greater", options: { value: 0 } } }]);

  return (
    <div className="grid gap-2 text-xs text-muted" aria-label="filter by value editor">
      <div className="flex items-center gap-2">
        <label className="flex items-center gap-1">
          Action
          <Select aria-label="filter type" className="h-7 w-24" value={type} onChange={(e) => onChange({ ...options, type: e.target.value })}>
            <option value="include">Include</option>
            <option value="exclude">Exclude</option>
          </Select>
        </label>
        <label className="flex items-center gap-1">
          Match
          <Select aria-label="filter match" className="h-7 w-20" value={match} onChange={(e) => onChange({ ...options, match: e.target.value })}>
            <option value="all">All</option>
            <option value="any">Any</option>
          </Select>
        </label>
      </div>

      {filters.length === 0 && <p>No conditions. Add one to filter rows by a field's value.</p>}
      {filters.map((f, idx) => {
        const id = f.config?.id ?? "greater";
        const kind = operandKind(id);
        const opts = f.config?.options ?? {};
        const setConfig = (nextId: string) => setFilter(idx, { ...f, config: { id: nextId, options: {} } });
        const setOperand = (patch: Record<string, unknown>) =>
          setFilter(idx, { ...f, config: { id, options: { ...opts, ...patch } } });
        return (
          <div key={idx} className="flex items-center gap-1.5" aria-label={`filter condition ${idx}`}>
            <FieldNamePicker aria-label={`filter ${idx} field`} className="w-32" value={f.fieldName ?? ""} onChange={(fieldName) => setFilter(idx, { ...f, fieldName })} />
            <Select aria-label={`filter ${idx} matcher`} className="h-7 w-36" value={id} onChange={(e) => setConfig(e.target.value)}>
              {MATCHERS.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.label}
                </option>
              ))}
            </Select>
            {kind === "number" && (
              <Input aria-label={`filter ${idx} value`} type="number" className="h-7 w-24 text-xs" value={typeof opts.value === "number" ? opts.value : ""} onChange={(e) => setOperand({ value: e.target.value === "" ? undefined : Number(e.target.value) })} />
            )}
            {kind === "text" && (
              <Input aria-label={`filter ${idx} value`} className="h-7 w-28 text-xs" value={opts.value == null ? "" : String(opts.value)} onChange={(e) => setOperand({ value: e.target.value })} />
            )}
            <Button variant="ghost" aria-label={`remove filter ${idx}`} className="h-auto px-1.5 hover:text-red-500" onClick={() => remove(idx)}>
              ×
            </Button>
          </div>
        );
      })}
      <Button variant="outline" size="sm" aria-label="add filter condition" className="h-auto justify-self-start px-2 py-0.5 text-[11px] hover:text-fg" onClick={add}>
        + Add condition
      </Button>
    </div>
  );
}
