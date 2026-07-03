// Shared option-control primitives for the per-view Panel-options editors (viz panel-editor scope). The
// per-view tabs (stat/gauge/bargauge/table/barchart/piechart) each compose these instead of repeating
// control markup. Built on the shadcn primitives (Select/Checkbox/Input) — the step-1 editor-parity
// burn-down of the old "no shadcn primitive yet" native controls. One responsibility: the labelled
// select / toggle / number controls.

import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";

/** A labelled select over a fixed option list. */
export function SelectField<T extends string>({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: T;
  options: readonly T[];
  onChange: (v: T) => void;
}) {
  return (
    <label className="grid gap-1 text-muted">
      {label}
      <Select aria-label={label} className="h-8" value={value} onChange={(e) => onChange(e.target.value as T)}>
        {options.map((o) => (
          <option key={o} value={o}>
            {o}
          </option>
        ))}
      </Select>
    </label>
  );
}

/** A labelled checkbox toggle. */
export function ToggleField({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <label className="flex items-center gap-2">
      <Checkbox aria-label={label} checked={checked} onChange={(e) => onChange(e.target.checked)} />
      {label}
    </label>
  );
}

/** A labelled number input (canonical units; the value is stored verbatim). */
export function NumberField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (v: number) => void;
}) {
  return (
    <label className="grid gap-1 text-muted">
      {label}
      <Input
        type="number"
        aria-label={label}
        className="h-8 text-xs"
        value={Number.isFinite(value) ? value : ""}
        onChange={(e) => onChange(Number(e.target.value))}
      />
    </label>
  );
}
