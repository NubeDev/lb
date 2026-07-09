// THE option registry (editor-parity scope, step 2) — the single aggregated list of every registered
// `OptionDef`, plus the lookups the tabs/pickers/search use. Adding an option = add its def to a `defs/*`
// file and (if a new file) import it here; nothing else in the editor changes. The registry-driven
// round-trip test iterates THIS list, so a new option can't dodge coverage. One responsibility: the
// aggregate + lookups.

import type { View } from "@/lib/dashboard";
import type { OptionDef } from "./types";
import { STANDARD_OPTIONS } from "./defs/standard";
import { TIMESERIES_GRAPH_OPTIONS } from "./defs/timeseriesGraph";
import { TIMESERIES_VIZ_OPTIONS } from "./defs/timeseriesViz";
import { TABLE_OPTIONS } from "./defs/table";
import { SINGLE_STAT_OPTIONS } from "./defs/singleStat";
import { INSIGHTS_OPTIONS } from "./defs/insights";

/** Views that carry NO fieldConfig — the universal standard field options (unit/decimals/thresholds…)
 *  are noise there and are excluded from their Options step. `insights` is a list widget, not a field
 *  render. Kept alongside the aggregation so a new fieldConfig-less view opts out in one place. */
const NO_FIELDCONFIG_VIEWS: View[] = ["insights", "weather"];

/** Every registered option, in tab/display order (standard first, then per-viz groups). The standard
 *  (universal) options are excluded from the fieldConfig-less views. */
export const OPTION_REGISTRY: OptionDef[] = [
  ...STANDARD_OPTIONS.map((d) => ({ ...d, excludeViews: [...(d.excludeViews ?? []), ...NO_FIELDCONFIG_VIEWS] })),
  ...TIMESERIES_GRAPH_OPTIONS,
  ...TIMESERIES_VIZ_OPTIONS,
  ...TABLE_OPTIONS,
  ...SINGLE_STAT_OPTIONS,
  ...INSIGHTS_OPTIONS,
];

/** The storage path for an option (its explicit `path` or, by default, its `id`). */
export function optionPath(def: OptionDef): string {
  return def.path ?? def.id;
}

/** Look up an option def by its id (the override property id). */
export function optionById(id: string): OptionDef | undefined {
  return OPTION_REGISTRY.find((d) => d.id === id);
}

/** Does this option apply to `view`? Universal options (`views` absent) apply everywhere EXCEPT a view
 *  in `excludeViews` (e.g. the standard fieldConfig options don't apply to the fieldConfig-less
 *  `insights` list); a scoped option applies only to its listed `views`. */
export function appliesToView(def: OptionDef, view: View): boolean {
  if (def.excludeViews?.includes(view)) return false;
  return !def.views || def.views.includes(view);
}

/** The options that apply to `view` (universal + view-scoped) — the set a tab renders + the override
 *  property picker offers for the current viz. */
export function optionsForView(view: View): OptionDef[] {
  return OPTION_REGISTRY.filter((d) => appliesToView(d, view));
}

/** Group options by their `group`, preserving registry order within and across groups. */
export function groupOptions(defs: OptionDef[]): Array<{ group: string; options: OptionDef[] }> {
  const out: Array<{ group: string; options: OptionDef[] }> = [];
  for (const def of defs) {
    const g = out.find((x) => x.group === def.group);
    if (g) g.options.push(def);
    else out.push({ group: def.group, options: [def] });
  }
  return out;
}
