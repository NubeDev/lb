// The variable bar (widget-config-vars Slice 2) — a dropdown per dashboard variable across the top of
// the dashboard. Query/source options resolve over the bridge (`useVariableOptions`); custom/interval
// are static; text is a free input; const is hidden (no control). The SELECTED value lives in the URL
// (`?var-<name>=`, repeated for multi) — this component reads the current selection from `selected` and
// writes changes up via `onChange` (the parent maps to `withVar` + a router navigate). Definitions on
// the record; selection in the URL (per-viewer, shareable).

import type { Variable } from "@/lib/vars";
import { useVariableOptions } from "./useVariableOptions";

interface Props {
  variables: Variable[];
  /** The current selection by bare variable name (from the URL). */
  selected: Record<string, string | string[]>;
  /** Write a variable's selection (the parent maps to `withVar` + navigate). */
  onChange: (name: string, value: string | string[] | undefined) => void;
  /** Bumped by auto-refresh (Slice 4) to re-resolve query variables. */
  refreshKey?: number;
}

const FIELD =
  "h-8 rounded-md border border-border bg-bg px-2.5 text-xs text-fg focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/20";

export function VariableBar({ variables, selected, onChange, refreshKey = 0 }: Props) {
  // `const` variables carry no control (hidden fixed value); everything else gets a labelled control.
  const visible = variables.filter((v) => v.type !== "const");
  if (visible.length === 0) return null;

  return (
    <div
      className="flex flex-wrap items-center gap-3 border-b border-border bg-panel px-3 py-2 text-xs"
      aria-label="variable bar"
    >
      {visible.map((v) => (
        <VariableControl
          key={v.name}
          variable={v}
          value={selected[v.name]}
          onChange={(val) => onChange(v.name, val)}
          refreshKey={refreshKey}
        />
      ))}
    </div>
  );
}

function VariableControl({
  variable,
  value,
  onChange,
  refreshKey,
}: {
  variable: Variable;
  value: string | string[] | undefined;
  onChange: (value: string | string[] | undefined) => void;
  refreshKey: number;
}) {
  const label = variable.label?.trim() || variable.name;

  // A text variable is a free input (no option list).
  if (variable.type === "text") {
    return (
      <label className="flex items-center gap-1.5">
        <span className="text-muted">{label}</span>
        {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Input variant here; token-bound */}
        <input
          aria-label={`variable ${variable.name}`}
          className={`${FIELD} w-36`}
          value={typeof value === "string" ? value : variable.text ?? ""}
          onChange={(e) => onChange(e.target.value || undefined)}
        />
      </label>
    );
  }

  return <SelectControl variable={variable} label={label} value={value} onChange={onChange} refreshKey={refreshKey} />;
}

const ALL_VALUE = "$__all";

function SelectControl({
  variable,
  label,
  value,
  onChange,
  refreshKey,
}: {
  variable: Variable;
  label: string;
  value: string | string[] | undefined;
  onChange: (value: string | string[] | undefined) => void;
  refreshKey: number;
}) {
  const { options, loading, denied } = useVariableOptions(variable, refreshKey);
  const current = Array.isArray(value) ? value : value !== undefined ? [value] : [];

  const onSelect = (e: React.ChangeEvent<HTMLSelectElement>) => {
    const picked = Array.from(e.target.selectedOptions, (o) => o.value);
    if (picked.includes(ALL_VALUE)) {
      onChange(variable.multi ? options.map((o) => o.value) : ALL_VALUE);
      return;
    }
    if (variable.multi) onChange(picked.length ? picked : undefined);
    else onChange(picked[0] || undefined);
  };

  return (
    <label className="flex items-center gap-1.5">
      <span className="text-muted">{label}</span>
      {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive; token-bound native */}
      <select
        aria-label={`variable ${variable.name}`}
        className={`${FIELD} ${variable.multi ? "h-auto min-h-8 w-44" : "w-44"}`}
        multiple={variable.multi}
        value={variable.multi ? current : current[0] ?? ""}
        onChange={onSelect}
      >
        {!variable.multi && <option value="">{loading ? "loading…" : denied ? "—" : "(none)"}</option>}
        {variable.includeAll && <option value={ALL_VALUE}>All</option>}
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
    </label>
  );
}
