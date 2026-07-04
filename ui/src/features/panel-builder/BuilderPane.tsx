// The INLINE panel builder (panel-builder views; data-studio scope v2/v3) — the full Grafana-style
// option surface (Query / Plot / Transform / Panel options / Field / Overrides, incl. the GenUI "AI
// widget" tab) rendered to FILL ITS PARENT, no modal chrome. The logic is the headless `usePanelEditor`
// state machine (`@/lib/panel-kit`); this file is only a VIEW over it: the live preview + viz picker and
// the options rail, plus save/save-as-library actions. Data Studio mounts one of these per FlexLayout
// builder tab (N side by side); the old dashboard modal `PanelEditor` is deleted — the dashboard only
// places library panels now.
//
// The `layout` prop chooses the arrangement of the two halves (v3):
//   - "split"   — preview+viz LEFT, options rail RIGHT (the default; dashboard-parity tests use it);
//   - "stacked" — the rendered PREVIEW on TOP (full-width), the options rail (Query first → the SQL
//     editor when the source needs it) BELOW. Data Studio's ONE stacked query/preview view: the user
//     sees the data and shapes the chart together, and an opened chart is the focus with its source
//     beneath it.

import { useEffect, useMemo, useState } from "react";
import { Check } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { Cell } from "@/lib/dashboard";
import { cellPrimaryTarget } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";
import { usePanelEditor } from "@/lib/panel-kit";
import { NavMenu, type NavItem } from "@nube/panel";

import { useVerticalSplit, SplitHandle } from "@/lib/split";

import { defaultOptionsForView } from "./viewOptions";
import { VizPicker } from "./VizPicker";
import { QueryStatusBar } from "./QueryStatusBar";
import { PreviewToolbar } from "./PreviewToolbar";
import { DataInspector } from "./DataInspector";
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
  /** The arrangement of the two halves (v3): "split" (preview LEFT, options RIGHT — the default) or
   *  "stacked" (preview on TOP, options BELOW — Data Studio's one stacked query/preview view). */
  layout?: "split" | "stacked";
}

const TAB_IDS = ["query", "plot", "transform", "options", "field", "overrides"] as const;
type TabId = (typeof TAB_IDS)[number];

export function BuilderPane({
  ws,
  cell,
  onSave,
  onDraftChange,
  scope = emptyScope(),
  saveLabel = "Save",
  layout = "split",
}: Props) {
  const stacked = layout === "stacked";
  const split = useVerticalSplit();
  const ed = usePanelEditor(cell, { defaultOptionsForView });
  const [tab, setTab] = useState<TabId>("query");
  const [search, setSearch] = useState("");
  // A table-view toggle on the preview: inspect the transformed frames as a table regardless of the
  // chosen viz, without changing the draft.
  const [tableView, setTableView] = useState(false);
  // Freeze (edit-without-requery): while frozen the datasource is NOT re-hit — option/source edits
  // reshape the frames already fetched. Unfreeze re-fetches once. Lets a user iterate against a slow query.
  const [frozen, setFrozen] = useState(false);
  // The data inspector drawer (Panel Inspect): frames / JSON / the resolved query.
  const [inspecting, setInspecting] = useState(false);

  // The draft's data — ONE read through the one data hook (invariant A) feeds BOTH the shape probe
  // (which views the picker offers) AND the result-field names every tab's field picker offers. `frozen`
  // routes to the fetch/shape split so a frozen preview reshapes without re-querying.
  const data = usePanelData(ed.draft, scope, ed.refreshKey, { frozen });
  const shape = data.loading ? "unknown" : detectShape(data.rows);
  const resultFields = useMemo(() => fieldNamesOf(data.rows), [data.rows]);
  const hasTarget = !!cellPrimaryTarget(ed.draft)?.tool;

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

  // The preview half — title + save actions, the library bar, the live preview, and the viz picker.
  // Stacked: the preview fills the top (grows); split: a fixed-height preview above the picker.
  const previewHalf = (
    <div className={`flex min-h-0 flex-col gap-3 ${stacked ? "h-full overflow-y-auto pr-1" : ""}`}>
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
      <PreviewToolbar
        hasTarget={hasTarget}
        loading={data.loading}
        frozen={frozen}
        onRun={ed.run}
        onToggleFreeze={() => setFrozen((f) => !f)}
        tableView={tableView}
        onToggleTableView={() => setTableView((v) => !v)}
        onInspect={() => setInspecting(true)}
      />
      <div className={stacked ? "min-h-[8rem] flex-1" : "h-56 shrink-0"}>
        <PreviewPane
          cell={ed.draft}
          ws={ws}
          scope={scope}
          refreshKey={ed.refreshKey}
          frozen={frozen}
          tableView={tableView}
        />
      </div>
      <QueryStatusBar state={data} hasTarget={hasTarget} frozen={frozen} />
      <VizPicker view={ed.viewC} onChange={ed.switchView} shape={shape} flowKind={ed.flowKind} />
    </div>
  );

  // The options half — Query first (the SQL editor surfaces here when the source needs it), then the
  // rest of the option surface beside the section nav.
  const optionsHalf = (
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
          onSelect={(id) => setTab(id as TabId)}
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

  // ⌘/Ctrl+Enter runs the query from anywhere in the builder (Grafana parity) — Run is no longer a
  // federation-only affordance. Ignored while frozen (nothing to fetch) or with no source.
  const onKeyDown = (e: React.KeyboardEvent) => {
    if ((e.metaKey || e.ctrlKey) && e.key === "Enter" && hasTarget && !frozen) {
      e.preventDefault();
      ed.run();
    }
  };

  return (
    <div aria-label="panel builder" className="flex h-full min-h-0 flex-col" onKeyDown={onKeyDown}>
      <DataInspector open={inspecting} onOpenChange={setInspecting} state={data} />
      <ResultFieldsProvider fields={resultFields}>
        {stacked ? (
          // Stacked (Data Studio v3): preview on TOP, options BELOW — one query/preview view. A draggable
          // divider sets how the height splits; each half scrolls internally so nothing is clipped.
          <div ref={split.containerRef} className="flex min-h-0 flex-1 flex-col overflow-hidden p-3">
            <div
              className="min-h-0 shrink-0 overflow-hidden"
              style={{ flexBasis: split.topBasis, pointerEvents: split.dragging ? "none" : undefined }}
            >
              {previewHalf}
            </div>
            {/* Drag up/down to shrink the preview and grow the options rail (and vice-versa). */}
            <SplitHandle onPointerDown={split.onHandleDown} label="resize preview and options" />
            <div
              className="min-h-0 flex-1 overflow-hidden"
              style={{ pointerEvents: split.dragging ? "none" : undefined }}
            >
              {optionsHalf}
            </div>
          </div>
        ) : (
          // Split (default): preview LEFT, options RIGHT.
          <div className="grid min-h-0 flex-1 grid-rows-[auto_1fr] gap-3 overflow-y-auto p-3 lg:grid-cols-[1.2fr_1fr] lg:grid-rows-1">
            {previewHalf}
            {optionsHalf}
          </div>
        )}
      </ResultFieldsProvider>
    </div>
  );
}
