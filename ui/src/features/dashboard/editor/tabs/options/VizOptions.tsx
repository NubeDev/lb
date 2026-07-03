// The registry-driven per-viz options body (editor-parity scope, step 5) — renders the `options`-scoped
// registry options for a view via OptionGroups, and composes the bespoke editors that are richer than a
// single control (the timeseries legend `calcs` chip row; the single-stat-family reduceOptions calc;
// the pie displayLabels multi-toggle). This is what brings each viz to everyday parity without
// rebuilding the whole surface. One responsibility: render a view's per-viz options.

import type { ReactNode } from "react";
import type { EditorState } from "../../cellEditorState";
import type { View } from "@/lib/dashboard";
import { optionsForView } from "../../options/registry";
import { OptionGroups } from "../../options/OptionGroups";

interface Props {
  view: View;
  state: EditorState;
  patch: (next: Partial<EditorState>) => void;
  search?: string;
  /** Bespoke controls that are richer than one registry control (legend calcs, reduce calc, pie labels),
   *  rendered under the registry groups. */
  extras?: ReactNode;
}

export function VizOptions({ view, state, patch, search, extras }: Props) {
  // Only the per-VIZ (`options`) registry options belong here; the `fieldConfig`-scoped ones are the
  // Field tab's job (Grafana's line). Table's per-column `custom.*` are fieldConfig → they show in the
  // table's Field tab, not here.
  const defs = optionsForView(view).filter((d) => d.scope === "options");
  return (
    <div aria-label="panel options tab">
      <OptionGroups defs={defs} state={state} patch={patch} search={search} />
      {extras}
    </div>
  );
}
