// The option-registry core types (editor-parity scope, step 2 — "an option registry, one property per
// file"). Grafana renders any property inside an override because options are REGISTERED (id, label,
// group, editor control, default), not hand-placed in tabs. We adopt that: each option is one
// `OptionDef` file describing WHAT it is (id/label/group), HOW it edits (a typed `control`), and WHERE
// it lives on the cell (a `scope` + dotted `path`). The Field tab, per-viz tabs, the overrides property
// picker, and options search all render FROM these defs. One responsibility: the def + control shapes.

import type { View } from "@/lib/dashboard";

/** The typed editor controls an option can use. Each maps to one renderer in `Control.tsx`; a new
 *  control kind is added here + there, once. `select`/`multi-select` carry their choice list. */
export type OptionControl =
  | { kind: "number"; min?: number; max?: number; step?: number; placeholder?: string }
  | { kind: "text"; placeholder?: string }
  | { kind: "toggle" }
  | { kind: "select"; choices: ReadonlyArray<{ value: string; label?: string; description?: string }> }
  | { kind: "multi-select"; choices: ReadonlyArray<{ value: string; label?: string }> }
  | { kind: "color" }
  | { kind: "unit" }
  | { kind: "field-name" }
  | { kind: "thresholds" }
  | { kind: "mappings" }
  | { kind: "color-scheme" }
  | { kind: "data-links" };

/** Where an option's value lives on the cell — the two roots the editor state carries. `fieldConfig`
 *  writes `fieldConfig.defaults.<path>` (and is what an OVERRIDE property sets per-field); `options`
 *  writes the per-viz `options.<path>`. A dotted `path` (e.g. `custom.lineWidth`) nests. */
export type OptionScope = "fieldConfig" | "options";

/** One registered option. The dotted `id` (Grafana-verbatim, e.g. `unit`, `custom.lineWidth`,
 *  `legend.showLegend`) is BOTH the registry key AND the override-property id, so an override that
 *  sets any option reuses its control for free (editor-parity scope, goal 4/5). */
export interface OptionDef {
  /** Dotted Grafana id — unique across the registry; the override property id. */
  id: string;
  /** Human label for the tab row + the override property picker. */
  label: string;
  /** The tab group this renders under (e.g. "Standard options", "Graph styles", "Axis", "Legend"). */
  group: string;
  /** Where it reads/writes on the cell. */
  scope: OptionScope;
  /** The dotted path under the scope root (defaults to `id` — most options are `id`-addressed). Set it
   *  when the property id and the storage path differ, or to keep `custom.*` ids tidy. */
  path?: string;
  control: OptionControl;
  /** The value a fresh cell / a newly-added override property is born with. */
  default: unknown;
  /** The views this option applies to. `undefined` = universal (a Standard option like unit/decimals);
   *  a list scopes a per-viz option (e.g. `custom.lineWidth` → timeseries). Drives the Field/per-viz
   *  tab rendering AND which options the override picker offers for the current viz. */
  views?: View[];
  /** Views this option is EXCLUDED from, even when it would otherwise be universal (`views` absent).
   *  For a view that carries no fieldConfig (e.g. `insights` — a list widget, not a field render), the
   *  standard field options (unit/decimals/thresholds…) are noise; excluding them keeps its Options step
   *  to the options that actually apply. Absent = no exclusion. */
  excludeViews?: View[];
  /** Keywords that also match this option in the options search (beyond label/id). */
  keywords?: string[];
}
