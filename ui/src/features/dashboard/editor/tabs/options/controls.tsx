// Shared option-control primitives for the per-view Panel-options editors (viz panel-editor scope). The
// per-view tabs (stat/gauge/bargauge/table/barchart/piechart) each compose these instead of repeating
// the native `<select>`/checkbox markup + its justified lint disable. One responsibility: the labelled
// select / toggle / number controls. (Native controls until shadcn Select/Checkbox primitives are
// generated — the documented `dashboard.md` follow-up; the disables are justified per the Phase-1
// precedent.)

const FIELD =
  "h-8 rounded-md border border-border bg-bg px-2.5 text-xs text-fg focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/20";

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
      {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive yet (dashboard.md follow-up) */}
      <select aria-label={label} className={FIELD} value={value} onChange={(e) => onChange(e.target.value as T)}>
        {options.map((o) => (
          <option key={o} value={o}>
            {o}
          </option>
        ))}
      </select>
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
      {/* eslint-disable-next-line no-restricted-syntax -- native checkbox; no shadcn Checkbox primitive (dashboard.md follow-up) */}
      <input type="checkbox" aria-label={label} checked={checked} onChange={(e) => onChange(e.target.checked)} />
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
      {/* eslint-disable-next-line no-restricted-syntax -- native number input; no shadcn numeric primitive (dashboard.md follow-up) */}
      <input
        type="number"
        aria-label={label}
        className={FIELD}
        value={Number.isFinite(value) ? value : ""}
        onChange={(e) => onChange(Number(e.target.value))}
      />
    </label>
  );
}
