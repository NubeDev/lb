// The INLINE panel builder (panel-builder views; data-studio scope v2) — the full Grafana-style option
// surface (Query / Plot / Transform / Panel options / Field / Overrides, incl. the GenUI "AI widget"
// tab) rendered to FILL ITS PARENT, no modal chrome. The logic is the headless `usePanelEditor` state
// machine (`@/lib/panel-kit`); this file is only a VIEW over it: live preview + viz picker on the left,
// the options rail on the right, save/save-as-library actions on top. Data Studio mounts one of these
// per FlexLayout builder tab (N side by side); the old dashboard modal `PanelEditor` is deleted — the
// dashboard only places library panels now.

import { useEffect, useMemo, useState } from "react";
import { Check } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { Cell } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";
import { usePanelEditor } from "@/lib/panel-kit";
import { NavMenu, type NavItem } from "@nube/panel";

import { defaultOptionsForView } from "./viewOptions";
import { VizPicker } from "./VizPicker";
import { usePanelData } from "@/features/dashboard/builder/usePanelData";
import { detectShape } from "@/features/dashboard/views/shape";
import { fieldNamesOf } from "./fields/resultFields";
import { ResultFieldsProvider } from "./fields/FieldsContext";
import { OptionsSearch } from "./OptionsSearch";
import { LibraryPanelBar } from "./LibraryPanelBar";
import { PreviewPane } from "./PreviewPane";
import { QueryTargets } from "./tabs/QueryTargets";
import { PlotAxesTab } from "./tabs/PlotAxesTab";
import { TransformTab } from "./tabs/TransformTab";
import { PanelOptionsTab } from "./tabs/PanelOptionsTab";
import { FieldTab } from "./tabs/FieldTab";
import { OverridesTab } from "./tabs/OverridesTab";

interface Props {
  ws: string;
  /** The cell to edit (EDIT) or a fresh default cell (ADD). Both enter the same (de)serializer. */
  cell: Cell;
  /** Persist the built cell. The host re-checks the save verb; any UI gate is convenience only. */
  onSave: (cell: Cell) => void;
  /** Notified on every draft change (Data Studio stows the draft in its FlexLayout tab config so the
   *  workbench survives reload). Optional — a consumer that only wants Save can omit it. */
  onDraftChange?: (cell: Cell) => void;
  scope?: VarScope;
  /** The Save button label ("Save" | "Save to library" …) — the consumer names the persistence. */
  saveLabel?: string;
}

const TAB_IDS = ["query", "plot", "transform", "options", "field", "overrides"] as const;
type TabId = (typeof TAB_IDS)[number];

export function BuilderPane({ ws, cell, onSave, onDraftChange, scope = emptyScope(), saveLabel = "Save" }: Props) {
  const ed = usePanelEditor(cell, { defaultOptionsForView });
  const [tab, setTab] = useState<TabId>("query");
  const [search, setSearch] = useState("");
  // A table-view toggle on the preview: inspect the transformed frames as a table regardless of the
  // chosen viz, without changing the draft.
  const [tableView, setTableView] = useState(false);

  // The draft's data — ONE read through the one data hook (invariant A) feeds BOTH the shape probe
  // (which views the picker offers) AND the result-field names every tab's field picker offers.
  const data = usePanelData(ed.draft, scope, ed.refreshKey);
  const shape = data.loading ? "unknown" : detectShape(data.rows);
  const resultFields = useMemo(() => fieldNamesOf(data.rows), [data.rows]);

  const patch = ed.patch;
  // Report every serialized draft AFTER the state applies (an inline `ed.toCell()` next to `patch`
  // would serialize the pre-patch state — one edit behind).
  useEffect(() => {
    onDraftChange?.(ed.draft);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- the draft is the only trigger
  }, [ed.draft]);

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
    <div aria-label="panel builder" className="flex h-full min-h-0 flex-col">
      <ResultFieldsProvider fields={resultFields}>
        <div className="grid min-h-0 flex-1 grid-rows-[auto_1fr] gap-3 overflow-y-auto p-3 lg:grid-cols-[1.2fr_1fr] lg:grid-rows-1">
          {/* Left: live preview + the viz picker (what the panel will look like). */}
          <div className="flex min-h-0 flex-col gap-3">
            <div className="flex items-center gap-2">
              <Input
                aria-label="panel title"
                className="h-8 text-sm"
                placeholder="Panel title"
                value={ed.state.title}
                onChange={(e) => patch({ title: e.target.value })}
              />
              <Button aria-label="save panel" size="sm" className="shrink-0" onClick={() => onSave(ed.toCell())}>
                <Check size={12} /> {saveLabel}
              </Button>
            </div>
            <LibraryPanelBar draft={ed.draft} onSave={onSave} />
            <div className="h-56 shrink-0">
              <PreviewPane
                cell={ed.draft}
                ws={ws}
                scope={scope}
                refreshKey={ed.refreshKey}
                tableView={tableView}
                onToggleTableView={() => setTableView((v) => !v)}
              />
            </div>
            <VizPicker view={ed.viewC} onChange={ed.switchView} shape={shape} flowKind={ed.flowKind} />
          </div>

          {/* Right: the options rail — search on top, then the NavMenu beside the active tab body. */}
          <div className="flex min-h-0 flex-col gap-2">
            <OptionsSearch value={search} onChange={setSearch} />
            <div className="grid min-h-0 flex-1 grid-cols-[9rem_1fr] gap-3">
              <NavMenu
                aria-label="panel builder sections"
                className="border-r border-border pr-2"
                items={tabItems}
                active={tab}
                badge={tabBadge}
                onSelect={(id) => setTab(id as TabId)}
              />
              <div className="min-h-0 flex-1 overflow-y-auto">
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
        </div>
      </ResultFieldsProvider>
    </div>
  );
}
