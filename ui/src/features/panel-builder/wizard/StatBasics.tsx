// StatBasics (panel-wizard scope, UX pass) — the stat view's BASIC setup on the chart-type step, the
// single-stat sibling of the plottable views' Plot section. Three things a stat author reaches for
// first, before the Options step's full set:
//   - Show sparkline — a plain switch over the registry's `graphMode` option (on → "area", the Grafana
//     default; off → "none"). The line/area nuance stays an advanced option.
//   - Thresholds + Value mappings — the SAME registry rows the Options step renders
//     (`OptionSectionCard` over `optionById`), so read/write goes through the ONE `readOption`/
//     `writeOption` binding — no second editor, no drift.
// One responsibility: the stat view's basic rows on the chart-type step.

import type { EditorState } from "@/lib/panel-kit/cellEditorState";
import { Switch } from "@/components/ui/switch";
import { optionById } from "@/features/panel-builder/options/registry";
import { readOption, writeOption } from "@/features/panel-builder/options/binding";
import { OptionSectionCard } from "@/features/panel-builder/options/OptionSectionCard";

interface Props {
  state: EditorState;
  patch: (next: Partial<EditorState>) => void;
}

/** The registry rows surfaced as stat basics (beyond the sparkline switch). */
const BASIC_OPTION_IDS = ["thresholds", "mappings"] as const;

export function StatBasics({ state, patch }: Props) {
  const graphDef = optionById("graphMode");
  const shown = graphDef ? (readOption(state, graphDef) ?? graphDef.default) !== "none" : true;
  const defs = BASIC_OPTION_IDS.map((id) => optionById(id)).filter((d) => d !== undefined);

  return (
    <div className="grid gap-2" aria-label="wizard stat basics">
      <div className="divide-y divide-border/60 overflow-hidden rounded-md border border-border bg-bg">
        <label className="flex items-center justify-between gap-2 px-3 py-2.5 text-xs">
          <span className="text-fg">Show sparkline</span>
          <Switch
            checked={shown}
            onCheckedChange={(on) => graphDef && patch(writeOption(state, graphDef, on ? "area" : "none"))}
            aria-label="show sparkline"
          />
        </label>
        {defs.map((def) => (
          <OptionSectionCard key={def.id} def={def} view="stat" state={state} patch={patch} />
        ))}
      </div>
    </div>
  );
}
