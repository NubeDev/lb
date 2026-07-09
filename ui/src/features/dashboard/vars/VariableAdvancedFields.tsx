// The advanced-variables authoring controls (advanced-variables scope) — regex extraction, option sort,
// refresh mode, custom "All" value, and hide mode. Split out of `VariableEditor` (FILE-LAYOUT: one
// responsibility per file; the editor was near the line budget). A collapsed row of compact controls,
// shown for every variable; the query-only ones (regex/sort/refresh) render only for a query variable.
//
// Each control writes an additive optional field on the one `Variable` — no per-type branch, no new
// record shape. Interpreted at resolve/render time by the resolver, interpolator, and bar.

import type {
  Variable,
  VariableHide,
  VariableRefresh,
  VariableSort,
  RegexApplyTo,
} from "@/lib/vars";

const FIELD =
  "h-8 w-full rounded-md border border-border bg-bg px-2.5 text-xs text-fg focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/20";

const SORTS: { v: VariableSort; label: string }[] = [
  { v: "none", label: "no sort" },
  { v: "alphaAsc", label: "alpha ↑" },
  { v: "alphaDesc", label: "alpha ↓" },
  { v: "numAsc", label: "numeric ↑" },
  { v: "numDesc", label: "numeric ↓" },
  { v: "alphaCiAsc", label: "alpha (ci) ↑" },
  { v: "alphaCiDesc", label: "alpha (ci) ↓" },
];

const REFRESHES: { v: VariableRefresh; label: string }[] = [
  { v: "onLoad", label: "on load" },
  { v: "onTimeRange", label: "on time range" },
  { v: "never", label: "never" },
];

const HIDES: { v: VariableHide; label: string }[] = [
  { v: "dontHide", label: "show" },
  { v: "hideLabel", label: "hide label" },
  { v: "hideVariable", label: "hide variable" },
];

export function VariableAdvancedFields({
  variable,
  onChange,
}: {
  variable: Variable;
  onChange: (patch: Partial<Variable>) => void;
}) {
  const isQuery = variable.type === "query" || variable.type === "source" || variable.type === "datasource";

  return (
    <div className="flex flex-col gap-3" aria-label="variable advanced fields">
      {isQuery && (
        <>
          <AdvField label="Regex extract / filter">
            {/* eslint-disable-next-line no-restricted-syntax -- token-bound native input */}
            <input
              aria-label="variable regex"
              className={FIELD}
              placeholder="(?<text>.+) \((?<value>[A-Z]+)\)"
              value={variable.regex ?? ""}
              onChange={(e) => onChange({ regex: e.target.value || undefined })}
            />
          </AdvField>
          <div className="flex flex-wrap gap-2">
            <AdvField label="Apply regex to">
              {/* eslint-disable-next-line no-restricted-syntax -- token-bound native select */}
              <select
                aria-label="variable regex apply to"
                className={FIELD}
                value={variable.regexApplyTo ?? "value"}
                onChange={(e) => onChange({ regexApplyTo: e.target.value as RegexApplyTo })}
              >
                <option value="value">value</option>
                <option value="text">text</option>
              </select>
            </AdvField>
            <AdvField label="Sort">
              {/* eslint-disable-next-line no-restricted-syntax -- token-bound native select */}
              <select
                aria-label="variable sort"
                className={FIELD}
                value={variable.sort ?? "none"}
                onChange={(e) => onChange({ sort: e.target.value as VariableSort })}
              >
                {SORTS.map((s) => (
                  <option key={s.v} value={s.v}>
                    {s.label}
                  </option>
                ))}
              </select>
            </AdvField>
            <AdvField label="Refresh">
              {/* eslint-disable-next-line no-restricted-syntax -- token-bound native select */}
              <select
                aria-label="variable refresh"
                className={FIELD}
                value={variable.refresh ?? "onLoad"}
                onChange={(e) => onChange({ refresh: e.target.value as VariableRefresh })}
              >
                {REFRESHES.map((r) => (
                  <option key={r.v} value={r.v}>
                    {r.label}
                  </option>
                ))}
              </select>
            </AdvField>
          </div>
        </>
      )}
      <div className="flex flex-wrap gap-2">
        {variable.includeAll && (
          <AdvField label="Custom “All” value">
            {/* eslint-disable-next-line no-restricted-syntax -- token-bound native input */}
            <input
              aria-label="variable all value"
              className={FIELD}
              placeholder=".*"
              value={variable.allValue ?? ""}
              onChange={(e) => onChange({ allValue: e.target.value || undefined })}
            />
          </AdvField>
        )}
        <AdvField label="Show on bar">
          {/* eslint-disable-next-line no-restricted-syntax -- token-bound native select */}
          <select
            aria-label="variable hide"
            className={FIELD}
            value={variable.hide ?? "dontHide"}
            onChange={(e) => onChange({ hide: e.target.value as VariableHide })}
          >
            {HIDES.map((h) => (
              <option key={h.v} value={h.v}>
                {h.label}
              </option>
            ))}
          </select>
        </AdvField>
      </div>
    </div>
  );
}

/** A labelled advanced field (small label above the control). */
function AdvField({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="flex min-w-[7rem] flex-1 flex-col gap-1">
      <span className="text-[11px] text-muted">{label}</span>
      {children}
    </label>
  );
}
