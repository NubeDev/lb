// The params form for a RULE source (rules-as-source scope, open Q1 + typed params) — one control per
// param the saved rule declared, TYPED by the param's `kind` (text/number/date/enum), writing the
// values into the `rules.run` target's `args.params`. Shown by the Query tab only when the primary
// target's tool is `rules.run`. A rule with no declared params renders nothing. The param list comes
// from the picker ENTRY (`entry.params`, carried by the package from `rules.list`) — never re-fetched
// here; a rule the picker didn't surface (denied `rules.list`) has no entry and so no form.
//
// One responsibility: edit a rule target's `params`. A NUMBER param rides as a JSON number (the host
// preserves the type into the cage — `param("n")` is a rhai number, not a string); text/date/enum ride
// as strings. An empty non-required field is omitted (the rule sees an absent param, its own default).
// A required-but-empty field is flagged (aria-invalid) so the author fixes it before the panel runs.

import type { ParamKind, RuleParam } from "@nube/source-picker";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import type { Target } from "@/lib/dashboard";

interface Props {
  /** The rule's declared params (from the picked entry). Empty → the section renders nothing. */
  params: RuleParam[];
  /** The primary target (`rules.run {rule_id, params}`) whose `args.params` this edits. */
  target: Target;
  /** Apply the target with the next `args.params`. */
  onChange: (next: Target) => void;
}

/** The current `params` map off a rule target (defaulting to empty). */
function paramsOf(target: Target): Record<string, unknown> {
  const p = target.args?.params;
  return p && typeof p === "object" ? (p as Record<string, unknown>) : {};
}

/** Coerce a raw input string to the JSON value the param's kind wants. A number rides as a JS number so
 *  the cage sees a rhai number; everything else rides as a string. An empty string → `undefined` (the
 *  caller omits the key). A non-finite number entry → `undefined` (treated as unset, not `NaN`). */
function coerce(kind: ParamKind, raw: string): unknown {
  if (raw === "") return undefined;
  if (kind === "number") {
    const n = Number(raw);
    return Number.isFinite(n) ? n : undefined;
  }
  return raw;
}

export function RuleParamsSection({ params, target, onChange }: Props) {
  if (params.length === 0) return null;
  const current = paramsOf(target);

  const setParam = (p: RuleParam, raw: string) => {
    const next = { ...current };
    const value = coerce(p.kind ?? "text", raw);
    if (value === undefined) delete next[p.name];
    else next[p.name] = value;
    onChange({ ...target, args: { ...target.args, params: next } });
  };

  return (
    <div className="grid gap-2" aria-label="rule params">
      {params.map((p) => {
        const kind = p.kind ?? "text";
        // The raw string shown in the control (a number param stores a JS number — stringify for display).
        const raw = current[p.name] === undefined ? "" : String(current[p.name]);
        const missing = p.required === true && raw === "";
        return (
          <label key={p.name} className="grid gap-1 text-xs text-muted">
            <span>
              {p.label || p.name}
              {p.required && <span className="text-danger"> *</span>}
            </span>
            {kind === "enum" ? (
              <Select
                aria-label={`rule param ${p.name}`}
                aria-invalid={missing || undefined}
                aria-required={p.required || undefined}
                className="h-8 w-full text-xs"
                value={raw}
                onChange={(e) => setParam(p, e.target.value)}
              >
                <option value="">{p.required ? "— choose —" : "— none —"}</option>
                {(p.options ?? []).map((o) => (
                  <option key={o} value={o}>
                    {o}
                  </option>
                ))}
              </Select>
            ) : (
              <Input
                aria-label={`rule param ${p.name}`}
                aria-invalid={missing || undefined}
                aria-required={p.required || undefined}
                className="h-8 w-full text-xs"
                type={kind === "number" ? "number" : kind === "date" ? "date" : "text"}
                value={raw}
                placeholder={p.name}
                onChange={(e) => setParam(p, e.target.value)}
              />
            )}
          </label>
        );
      })}
    </div>
  );
}
