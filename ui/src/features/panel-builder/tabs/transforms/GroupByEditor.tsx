// The groupBy transform editor (editor-parity scope, step 3) — Grafana's per-field rows over the real
// result fields: each field is Group by / Calculate / Ignore, and Calculate reveals the aggregations.
// Backend shape verbatim (`rust/crates/viz` group_by): `fields: Record<name, {operation, aggregations}>`
// where `operation` is "groupby" | "aggregate" | (ignore = absent), `aggregations: ReducerID[]`. One
// responsibility: author a groupBy config.

import { Button } from "@/components/ui/button";
import { Select } from "@/components/ui/select";
import { useResultFields } from "../../fields/FieldsContext";

interface Props {
  options: Record<string, unknown>;
  onChange: (options: Record<string, unknown>) => void;
}

interface FieldCfg {
  operation?: "groupby" | "aggregate";
  aggregations?: string[];
}
type FieldsMap = Record<string, FieldCfg>;

const AGGS = ["sum", "mean", "min", "max", "count", "last", "first"];

export function GroupByEditor({ options, onChange }: Props) {
  const resultFields = useResultFields();
  const fields: FieldsMap = options.fields && typeof options.fields === "object" ? (options.fields as FieldsMap) : {};
  // Show every real field, plus any field already configured (a saved config over data not yet previewed).
  const names = [...new Set([...resultFields, ...Object.keys(fields)])];

  const setField = (name: string, cfg: FieldCfg | undefined) => {
    const next: FieldsMap = { ...fields };
    if (cfg) next[name] = cfg;
    else delete next[name];
    onChange({ ...options, fields: next });
  };
  const op = (name: string): "groupby" | "aggregate" | "ignore" => fields[name]?.operation ?? "ignore";
  const toggleAgg = (name: string, agg: string) => {
    const cur = fields[name]?.aggregations ?? [];
    const aggregations = cur.includes(agg) ? cur.filter((a) => a !== agg) : [...cur, agg];
    setField(name, { operation: "aggregate", aggregations });
  };

  if (names.length === 0) {
    return <p className="text-xs text-muted">No fields yet — run the query to list the result fields.</p>;
  }

  return (
    <div className="grid gap-1.5 text-xs text-muted" aria-label="group by editor">
      {names.map((name) => {
        const operation = op(name);
        return (
          <div key={name} className="grid gap-1" aria-label={`group by field ${name}`}>
            <div className="flex items-center gap-1.5">
              <span className="w-28 shrink-0 truncate font-mono text-[11px] text-fg">{name}</span>
              <Select
                aria-label={`${name} operation`}
                className="h-7 w-32"
                value={operation}
                onChange={(e) => {
                  const v = e.target.value as "groupby" | "aggregate" | "ignore";
                  if (v === "ignore") setField(name, undefined);
                  else if (v === "groupby") setField(name, { operation: "groupby" });
                  else setField(name, { operation: "aggregate", aggregations: fields[name]?.aggregations ?? ["sum"] });
                }}
              >
                <option value="ignore">Ignore</option>
                <option value="groupby">Group by</option>
                <option value="aggregate">Calculate</option>
              </Select>
            </div>
            {operation === "aggregate" && (
              <div className="flex flex-wrap gap-1 pl-[7.5rem]">
                {AGGS.map((agg) => {
                  const on = (fields[name]?.aggregations ?? []).includes(agg);
                  return (
                    <Button
                      key={agg}
                      type="button"
                      size="sm"
                      variant={on ? "default" : "outline"}
                      aria-label={`${name} agg ${agg}`}
                      aria-pressed={on}
                      className="h-auto px-2 py-0.5 text-[11px]"
                      onClick={() => toggleAgg(name, agg)}
                    >
                      {agg}
                    </Button>
                  );
                })}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
