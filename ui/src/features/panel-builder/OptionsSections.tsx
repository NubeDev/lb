// The builder's option surface (extracted from `BuilderPane` — data-studio-10x scope, phase 3): the
// searchable section nav (Query / Plot / Transform / Panel options / Field / Overrides) beside the
// active section body. The full Grafana-depth surface, unchanged — phase 3 reorganizes only WHEN it
// appears (inline in the split layout; inside the collapsed `OptionsDrawer` in the query-first
// stacked flow), not what it is. One responsibility: the option sections.

import { useState } from "react";

import type { VarScope } from "@/lib/vars";
import type { usePanelEditor } from "@/lib/panel-kit";
import { NavMenu, type NavItem } from "@nube/panel";

import { OptionsSearch } from "./OptionsSearch";
import { QueryTargets } from "./tabs/QueryTargets";
import { PlotAxesTab } from "./tabs/PlotAxesTab";
import { TransformTab } from "./tabs/TransformTab";
import { PanelOptionsTab } from "./tabs/PanelOptionsTab";
import { FieldTab } from "./tabs/FieldTab";
import { OverridesTab } from "./tabs/OverridesTab";

const TAB_IDS = ["query", "plot", "transform", "options", "field", "overrides"] as const;
export type BuilderTabId = (typeof TAB_IDS)[number];

interface Props {
  ws: string;
  ed: ReturnType<typeof usePanelEditor>;
  scope: VarScope;
}

export function OptionsSections({ ws, ed, scope }: Props) {
  const [tab, setTab] = useState<BuilderTabId>("query");
  const [search, setSearch] = useState("");
  const patch = ed.patch;

  const tabItems: NavItem[] = [
    { id: "query", label: "Query" },
    ...(ed.canPlot ? [{ id: "plot", label: "Plot" } as NavItem] : []),
    { id: "transform", label: "Transform" },
    { id: "options", label: "Panel options" },
    { id: "field", label: "Field" },
    { id: "overrides", label: "Overrides" },
  ];
  const tabBadge = (id: string): number | undefined => {
    if (id === "transform") return ed.state.transformations.length || undefined;
    if (id === "overrides") return ed.state.fieldConfig?.overrides?.length || undefined;
    return undefined;
  };

  return (
    <div className="flex h-full min-h-0 flex-col gap-2">
      <OptionsSearch value={search} onChange={setSearch} />
      {/* `grid-rows-[minmax(0,1fr)]` is load-bearing: a grid track is content-sized (`auto`) by default,
          so without an explicit minmax(0,…) row the section content grows the track instead of scrolling —
          the tall option tabs (Plot/Field/Overrides) then overflow the pane with NO scrollbar. The
          minmax(0,1fr) caps the row at the available height so the inner `overflow-y-auto` engages. */}
      <div className="grid min-h-0 flex-1 grid-cols-[9rem_1fr] grid-rows-[minmax(0,1fr)] gap-3">
        <NavMenu
          aria-label="panel builder sections"
          className="min-h-0 overflow-y-auto border-r border-border pr-2"
          items={tabItems}
          active={tab}
          badge={tabBadge}
          onSelect={(id) => setTab(id as BuilderTabId)}
        />
        <div className="min-h-0 overflow-y-auto">
          {tab === "query" && <QueryTargets ws={ws} state={ed.state} patch={patch} onRun={ed.run} />}
          {tab === "plot" && ed.canPlot && (
            <PlotAxesTab draft={ed.draft} state={ed.state} patch={patch} scope={scope} refreshKey={ed.refreshKey} />
          )}
          {tab === "transform" && (
            <TransformTab state={ed.state} patch={patch} draft={ed.draft} scope={scope} refreshKey={ed.refreshKey} />
          )}
          {tab === "options" && <PanelOptionsTab state={ed.stateC} patch={patch} ws={ws} />}
          {tab === "field" && <FieldTab state={ed.stateC} patch={patch} search={search} />}
          {tab === "overrides" && <OverridesTab state={ed.state} patch={patch} />}
        </div>
      </div>
    </div>
  );
}
