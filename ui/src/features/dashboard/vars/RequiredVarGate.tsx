// The required-variable gate (reusable-pages scope, "the make-or-break UX"). A **template** dashboard
// is an ordinary dashboard with one or more `required` variables (page parameters). When such a
// variable is UNBOUND — no `?var-` URL selection AND no default in the resolved scope — the dashboard
// must NOT fire its cells: a `series.read { series: "hvac.${site}.temp" }` with `site` unresolved would
// splice a `$site`-literal query and render a wall of broken/empty charts. Instead the view renders
// THIS honest "select a `<label>`" empty state, with the variable bar lit above it, until the parameter
// is picked. The gate holds cell firing **before any bridge call** (it replaces the `<Grid>`), which is
// the invariant the gateway test asserts (zero cell calls while unbound).
//
// One responsibility per file (FILE-LAYOUT): compute the unbound-required set + render the gate; the
// caller (DashboardView) swaps the grid for it.

import { SlidersHorizontal } from "lucide-react";

import type { Variable, VarScope } from "@/lib/vars";

/** The `required` variables that are still UNBOUND in `scope` — no URL selection, no default resolved.
 *  An empty list means the template is fully bound and its cells may fire. */
export function unboundRequiredVars(variables: Variable[], scope: VarScope): Variable[] {
  return variables.filter((v) => {
    if (!v.required) return false;
    const val = scope.values[v.name];
    if (val === undefined) return true;
    // A multi-select resolving to an empty list is still "unbound" (nothing to interpolate).
    if (Array.isArray(val)) return val.length === 0;
    return val === "";
  });
}

/** A label for a variable (its `label`, else its `name`). */
function labelOf(v: Variable): string {
  return v.label && v.label.trim() ? v.label : v.name;
}

interface Props {
  /** The unbound required variables (from {@link unboundRequiredVars}) — never empty when this renders. */
  unbound: Variable[];
}

/** The waiting empty-state shown in place of the grid while a required page parameter is unbound. It
 *  names the parameter(s) to pick — the variable bar above it carries the actual dropdowns. */
export function RequiredVarGate({ unbound }: Props) {
  const names = unbound.map(labelOf);
  const which =
    names.length === 1
      ? `a ${names[0]}`
      : `${names.slice(0, -1).join(", ")} and ${names[names.length - 1]}`;
  return (
    <div
      role="status"
      aria-label="select a page parameter"
      data-testid="required-var-gate"
      className="flex min-h-0 flex-1 flex-col items-center justify-center py-16 text-center"
    >
      <div className="flex h-11 w-11 items-center justify-center rounded-xl border border-border bg-bg text-accent">
        <SlidersHorizontal size={18} />
      </div>
      <p className="mt-3 text-sm font-medium text-fg">Select {which} to load this page.</p>
      <p className="mt-1 max-w-sm text-xs leading-5 text-muted">
        This is a template page — pick{" "}
        {names.length === 1 ? "the parameter" : "each parameter"} in the bar above and its widgets will
        load, scoped to your selection.
      </p>
    </div>
  );
}
