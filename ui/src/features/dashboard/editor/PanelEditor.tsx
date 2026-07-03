// The ONE Grafana-style panel editor (viz panel-editor scope) — the single surface for ADD and EDIT.
// It mounts on a cell, reconstructs the COMPLETE option model via the one `cellToEditorState`
// (de)serializer, and renders: a live preview + the viz picker + the options rail (Query / Transform /
// Panel options / Field / Overrides) with an options search. Save serializes back via
// `editorStateToCell`, PRESERVING the cell key + geometry, and calls `onSave` (→ `dashboard.save`). Add
// and edit are the SAME component, the SAME path — so the option surface and the round-trip are provably
// identical and the user's "edit loses my SQL options / add ≠ edit" bug is impossible.
//
// One responsibility: the editor shell + state + dispatch. Each tab, the picker, the preview, the
// (de)serializer live in their own files (FILE-LAYOUT); this composes them.

import { useMemo, useState } from "react";
import { Check } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { Cell, View } from "@/lib/dashboard";
import { canonicalView } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";

import { cellToEditorState, editorStateToCell, type EditorState } from "./cellEditorState";
// The reusable, resizable ce-wiresheet-style panel (replaces the old cramped fixed-width
// Sheet). Its NavMenu is the options rail. Widening the panel reveals more of the option
// bodies — the "so many options on resize" behavior the CE panel has.
import { Panel, NavMenu, type NavItem } from "@nube/panel";

import { defaultOptionsForView } from "./viewOptions";
import { VizPicker } from "./VizPicker";
import { usePanelData } from "../builder/usePanelData";
import { detectShape } from "../views/shape";
import { fieldNamesOf } from "./fields/resultFields";
import { ResultFieldsProvider } from "./fields/FieldsContext";
import { OptionsSearch } from "./OptionsSearch";
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
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Persist the edited cell (the parent splices it into the layout + saves the whole dashboard). The
   *  host re-checks `dashboard.save`; the editor's mere presence is gated on the edit cap by the caller. */
  onSave: (cell: Cell) => void;
  scope?: VarScope;
}

const TAB_IDS = ["query", "plot", "transform", "options", "field", "overrides"] as const;
type TabId = (typeof TAB_IDS)[number];

/** The cartesian chart views that support the shared X/Y plot builder (the Plot tab). */
const PLOTTABLE_VIEWS = new Set(["timeseries", "barchart", "piechart"]);

export function PanelEditor({ ws, cell, open, onOpenChange, onSave, scope = emptyScope() }: Props) {
  // The whole working state — rebuilt from the cell via the ONE (de)serializer. Re-seeded when the
  // edited cell identity changes (a new Add/Edit target).
  const [state, setState] = useState<EditorState>(() => cellToEditorState(cell));
  const [seededFor, setSeededFor] = useState(cell.i);
  if (open && seededFor !== cell.i) {
    setState(cellToEditorState(cell));
    setSeededFor(cell.i);
  }
  const [tab, setTab] = useState<TabId>("query");
  const [search, setSearch] = useState("");
  // An explicit "Run" nonce: the Query tab's Run button bumps it to force a preview re-query even when
  // the spec is byte-identical (re-running the same SQL). It's folded into the refresh tick below.
  const [runNonce, setRunNonce] = useState(0);
  // A table-view toggle on the preview: inspect the transformed frames as a table regardless of the
  // chosen viz (editor-parity step 6). Renders the draft through the `table` view without changing the
  // saved cell.
  const [tableView, setTableView] = useState(false);
  const run = () => setRunNonce((n) => n + 1);
  // A debounce-ish tick so the preview re-queries when the source changes (cheap: state edits are rare),
  // plus the explicit Run nonce so pressing Run always re-fires the query.
  const refreshKey = useMemo(
    () => JSON.stringify(state.targets).length + (state.sql?.rawSql.length ?? 0) + runNonce,
    [state.targets, state.sql, runNonce],
  );

  const patch = (next: Partial<EditorState>) => setState((s) => ({ ...s, ...next }));
  // The view canonicalized for DISPLAY (picker highlight + per-view tab branching). The stored
  // `state.view` stays raw so a v2 `chart` cell serializes byte-identical; here we show it as its
  // canonical `timeseries`. A new/empty cell defaults to `timeseries`.
  const viewC = canonicalView((state.view || "timeseries") as View);
  const stateC = { ...state, view: viewC };

  // Switching the viz preserves targets/fieldConfig/transformations; only the per-view `options` reset
  // to that view's Grafana defaults (panel-editor scope, Risks: "viz switch must preserve compatible
  // state"). The carry-over set is exactly: targets, sql, fieldConfig, transformations, title.
  const switchView = (view: View) =>
    setState((s) => ({ ...s, view, options: defaultOptionsForView(view) }));

  // A Flows binding swaps the viz picker's offered set (flow-dashboard-binding-ux-scope): an INPUT
  // port carries a `flows.inject` action (write controls); an OUTPUT port reads `flows.node_state`
  // (the JSON read view + scalar viz). Absent → the standard viz set.
  const flowKind: "input" | "output" | null =
    state.carry.action?.tool === "flows.inject"
      ? "input"
      : state.targets[0]?.tool === "flows.node_state"
        ? "output"
        : null;

  // The draft cell = what save would persist (also the preview's input). Built from the SAME serializer.
  const draft = useMemo(() => editorStateToCell(state, cell), [state, cell]);

  // The draft's data — ONE read through the one data hook (invariant A; no separate fetch) feeds BOTH
  // the shape probe (which views the picker offers) AND the result-field names every tab's field
  // picker offers (editor-parity step 1: the editor OFFERS real field names, never makes you retype).
  const data = usePanelData(draft, scope, refreshKey);
  const shape = data.loading ? "unknown" : detectShape(data.rows);
  const resultFields = useMemo(() => fieldNamesOf(data.rows), [data.rows]);

  const save = () => {
    onSave(editorStateToCell(state, cell));
    onOpenChange(false);
  };

  const canPlot = PLOTTABLE_VIEWS.has(viewC);
  // The options rail is the reusable NavMenu (re-exported by @nube/panel from @nube/nav-rail;
  // one data-driven vertical nav, section ids = tab ids. Badges (transform/override counts) come
  // through NavMenu's `badge` fn below.
  const tabItems: NavItem[] = [
    { id: "query", label: "Query" },
    ...(canPlot ? [{ id: "plot", label: "Plot" } as NavItem] : []),
    { id: "transform", label: "Transform" },
    { id: "options", label: "Panel options" },
    { id: "field", label: "Field" },
    { id: "overrides", label: "Overrides" },
  ];
  const tabBadge = (id: string): number | undefined => {
    if (id === "transform") return state.transformations.length || undefined;
    if (id === "overrides") return state.fieldConfig?.overrides?.length || undefined;
    return undefined;
  };

  return (
    <Panel
      open={open}
      onOpenChange={onOpenChange}
      aria-label="panel editor"
      title="Edit panel"
      description="One editor for add and edit — the full Grafana option surface."
      // Wide + resizable (was a fixed `sm:max-w-3xl` Sheet): drag the left edge to widen
      // and the two-pane preview/options layout gets more room, revealing more option
      // columns — the ce-wiresheet "so many options on resize" behavior.
      initialWidth={960}
      minWidth={560}
      maxWidth={1400}
      footer={
        <>
          <Button variant="ghost" size="sm" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button aria-label="save panel" size="sm" onClick={save}>
            <Check size={12} /> Save
          </Button>
        </>
      }
    >
      <ResultFieldsProvider fields={resultFields}>
      <div className="grid min-h-0 flex-1 grid-rows-[auto_1fr] gap-3 overflow-y-auto p-4 lg:grid-cols-[1.2fr_1fr] lg:grid-rows-1">
        {/* Left: live preview + the viz picker (what the panel will look like). */}
        <div className="flex min-h-0 flex-col gap-3">
          <Input
            aria-label="panel title"
            className="h-8 text-sm"
            placeholder="Panel title"
            value={state.title}
            onChange={(e) => patch({ title: e.target.value })}
          />
          <div className="h-56 shrink-0">
            <PreviewPane cell={draft} ws={ws} scope={scope} refreshKey={refreshKey} tableView={tableView} onToggleTableView={() => setTableView((v) => !v)} />
          </div>
          <VizPicker view={viewC} onChange={switchView} shape={shape} flowKind={flowKind} />
        </div>

        {/* Right: the options rail — search on top, then the reusable NavMenu (Query/Transform/…)
            beside the active tab body. */}
        <div className="flex min-h-0 flex-col gap-2">
          <OptionsSearch value={search} onChange={setSearch} />
          <div className="grid min-h-0 flex-1 grid-cols-[9rem_1fr] gap-3">
            <NavMenu
              aria-label="panel editor sections"
              className="border-r border-border pr-2"
              items={tabItems}
              active={tab}
              badge={tabBadge}
              onSelect={(id) => setTab(id as TabId)}
            />
            <div className="min-h-0 flex-1 overflow-y-auto">
              {tab === "query" && <QueryTargets ws={ws} state={state} patch={patch} onRun={run} />}
              {tab === "plot" && canPlot && (
                <PlotAxesTab draft={draft} state={state} patch={patch} scope={scope} refreshKey={refreshKey} />
              )}
              {tab === "transform" && <TransformTab state={state} patch={patch} />}
              {tab === "options" && <PanelOptionsTab state={stateC} patch={patch} ws={ws} />}
              {tab === "field" && <FieldTab state={stateC} patch={patch} search={search} />}
              {tab === "overrides" && <OverridesTab state={state} patch={patch} />}
            </div>
          </div>
        </div>
      </div>
      </ResultFieldsProvider>
    </Panel>
  );
}
