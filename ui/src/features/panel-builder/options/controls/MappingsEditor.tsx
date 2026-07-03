// The value-mappings editor (editor-parity scope, step 2/3 goal) — authors `fieldConfig.defaults.mappings`
// as typed rows (value / range / special → display text + color), the exact shape the render path
// (`fieldconfig/mappings.ts`) ALREADY applies. Before this, authored mappings were invisible-to-edit.
// Grafana's three editable mapping kinds:
//   - value   → an exact match on a stringified value  → {text,color}
//   - range   → a numeric [from,to] window             → {text,color}
//   - special → null/nan/empty/true/false              → {text,color}
// (`regex` stays import-only — the render path defers it; not offered for authoring here.)
// One responsibility: edit the value-mappings list.

import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { ColorSwatchPicker } from "@/components/ui/color-swatch";
import type { ValueMapping, ValueMappingResult } from "@/lib/dashboard";
import { COLOR_SWATCHES } from "../palette";

interface Props {
  value: ValueMapping[] | undefined;
  onChange: (next: ValueMapping[] | undefined) => void;
}

/** A flat editing row — one row per mapping (a `value` mapping edits its FIRST key here; Grafana groups
 *  many exact values under one `value` mapping, but a per-row model is the friendlier authoring shape
 *  and serializes back to the same union). */
type Row =
  | { kind: "value"; match: string; result: ValueMappingResult }
  | { kind: "range"; from: string; to: string; result: ValueMappingResult }
  | { kind: "special"; match: "null" | "nan" | "empty" | "true" | "false"; result: ValueMappingResult };

/** Explode the stored union into editing rows (a multi-key `value` mapping → one row per key). */
function toRows(mappings: ValueMapping[]): Row[] {
  const rows: Row[] = [];
  for (const m of mappings) {
    if (m.type === "value") {
      for (const [match, result] of Object.entries(m.options)) rows.push({ kind: "value", match, result });
    } else if (m.type === "range") {
      rows.push({
        kind: "range",
        from: m.options.from == null ? "" : String(m.options.from),
        to: m.options.to == null ? "" : String(m.options.to),
        result: m.options.result,
      });
    } else if (m.type === "special") {
      rows.push({ kind: "special", match: m.options.match, result: m.options.result });
    }
    // `regex` rows are import-only (render defers them) — not surfaced for editing, but preserved by
    // leaving them out here would DROP them; so we keep them verbatim via `carryRegex` below.
  }
  return rows;
}

/** Rebuild the stored union from rows. Consecutive `value` rows fold into one `value` mapping (Grafana's
 *  shape); range/special stay one mapping each. `regex` mappings are carried through untouched. */
function fromRows(rows: Row[], carryRegex: ValueMapping[]): ValueMapping[] {
  const out: ValueMapping[] = [];
  let valueBucket: Record<string, ValueMappingResult> | null = null;
  const flush = () => {
    if (valueBucket && Object.keys(valueBucket).length) out.push({ type: "value", options: valueBucket });
    valueBucket = null;
  };
  for (const r of rows) {
    if (r.kind === "value") {
      valueBucket ??= {};
      if (r.match !== "") valueBucket[r.match] = r.result;
    } else if (r.kind === "range") {
      flush();
      out.push({
        type: "range",
        options: {
          from: r.from === "" ? null : Number(r.from),
          to: r.to === "" ? null : Number(r.to),
          result: r.result,
        },
      });
    } else {
      flush();
      out.push({ type: "special", options: { match: r.match, result: r.result } });
    }
  }
  flush();
  return [...out, ...carryRegex];
}

export function MappingsEditor({ value, onChange }: Props) {
  const mappings = value ?? [];
  const carryRegex = mappings.filter((m) => m.type === "regex");
  // The rows are LOCAL state: an in-progress row (blank match / blank range bound) can't be represented
  // in the stored `ValueMapping[]`, so deriving rows purely from `value` would drop a row the instant
  // it's added. We seed from `value` once, edit rows freely, and serialize only the complete ones out.
  const [rows, setRows] = useState<Row[]>(() => toRows(mappings));

  const write = (next: Row[]) => {
    setRows(next);
    const built = fromRows(next, carryRegex);
    onChange(built.length ? built : undefined);
  };
  const setRow = (idx: number, row: Row) => write(rows.map((r, i) => (i === idx ? row : r)));
  const setResult = (idx: number, next: Partial<ValueMappingResult>) => {
    const r = rows[idx];
    setRow(idx, { ...r, result: { ...r.result, ...next } } as Row);
  };
  const remove = (idx: number) => write(rows.filter((_, i) => i !== idx));
  const add = (kind: Row["kind"]) => {
    const blank: ValueMappingResult = { text: "" };
    const row: Row =
      kind === "value"
        ? { kind: "value", match: "", result: blank }
        : kind === "range"
          ? { kind: "range", from: "", to: "", result: blank }
          : { kind: "special", match: "null", result: blank };
    write([...rows, row]);
  };

  return (
    <div className="grid gap-2" aria-label="value mappings editor">
      {rows.length === 0 && <p className="text-xs text-muted">No value mappings.</p>}
      {rows.map((r, idx) => (
        <div key={idx} className="grid gap-1.5 rounded-md border border-border bg-bg p-2" aria-label={`mapping ${idx}`}>
          <div className="flex items-center gap-1.5">
            <Select
              aria-label={`mapping ${idx} kind`}
              className="h-7 w-24"
              value={r.kind}
              onChange={(e) => {
                const kind = e.target.value as Row["kind"];
                if (kind === r.kind) return;
                setRow(
                  idx,
                  kind === "value"
                    ? { kind: "value", match: "", result: r.result }
                    : kind === "range"
                      ? { kind: "range", from: "", to: "", result: r.result }
                      : { kind: "special", match: "null", result: r.result },
                );
              }}
            >
              <option value="value">Value</option>
              <option value="range">Range</option>
              <option value="special">Special</option>
            </Select>
            {r.kind === "value" && (
              <Input
                aria-label={`mapping ${idx} match`}
                className="h-7 flex-1 text-xs"
                placeholder="exact value"
                value={r.match}
                onChange={(e) => setRow(idx, { ...r, match: e.target.value })}
              />
            )}
            {r.kind === "range" && (
              <>
                <Input aria-label={`mapping ${idx} from`} type="number" className="h-7 w-20 text-xs" placeholder="from" value={r.from} onChange={(e) => setRow(idx, { ...r, from: e.target.value })} />
                <Input aria-label={`mapping ${idx} to`} type="number" className="h-7 w-20 text-xs" placeholder="to" value={r.to} onChange={(e) => setRow(idx, { ...r, to: e.target.value })} />
              </>
            )}
            {r.kind === "special" && (
              <Select
                aria-label={`mapping ${idx} special`}
                className="h-7 flex-1"
                value={r.match}
                onChange={(e) => setRow(idx, { ...r, match: e.target.value as typeof r.match })}
              >
                {["null", "nan", "empty", "true", "false"].map((m) => (
                  <option key={m} value={m}>
                    {m}
                  </option>
                ))}
              </Select>
            )}
            <Button variant="ghost" aria-label={`remove mapping ${idx}`} className="h-auto px-1.5 text-muted hover:text-red-500" onClick={() => remove(idx)}>
              ×
            </Button>
          </div>
          <div className="flex items-center gap-1.5">
            <Input
              aria-label={`mapping ${idx} text`}
              className="h-7 flex-1 text-xs"
              placeholder="display text"
              value={r.result.text ?? ""}
              onChange={(e) => setResult(idx, { text: e.target.value || undefined })}
            />
            <ColorSwatchPicker
              aria-label={`mapping ${idx} color`}
              palette={COLOR_SWATCHES}
              value={r.result.color ?? ""}
              onChange={(color) => setResult(idx, { color: color || undefined })}
            />
          </div>
        </div>
      ))}
      <div className="flex gap-1.5">
        {(["value", "range", "special"] as const).map((k) => (
          <Button key={k} variant="outline" size="sm" aria-label={`add ${k} mapping`} className="h-auto px-2 py-0.5 text-[11px] text-muted hover:text-fg" onClick={() => add(k)}>
            + {k}
          </Button>
        ))}
      </div>
    </div>
  );
}
