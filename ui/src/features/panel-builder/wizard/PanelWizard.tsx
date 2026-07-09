// The stepped panel wizard (panel-wizard scope, step 4) — the create-flow companion to the editor. A
// thin shell over the EXISTING panel model: the working state IS `EditorState` (seeded from
// `defaultCell("timeseries")`); every step writes through `patch`; the full-panel preview renders
// through the SAME `PreviewPane`/`WidgetView` the editor uses. There is no wizard-only state — drift
// between create and edit is impossible by construction (the load-bearing rule).
//
// Step 4 wires the route + SourceStep + ChartTypeStep + the full-panel preview beside them. The
// OptionsStep, TransformStep, the joyride tour, and Save land in subsequent steps (each its own commit,
// each green). A "skip to section" nav keeps the steps addressable, not forced.
//
// Layout: a resizable split (useSplitPane) — the active step on the left, the pinned `WizardPreview`
// (chart | table | JSON, one render path) on the right, a draggable separator between them. The step
// list + the dashboard id (the save target) thread through props; the wizard owns no persistence — Save
// (step 8) calls `editorStateToCell` → `dashboard.save`.
//
// One responsibility: orchestrate the wizard's EditorState + step transitions + layout.

import { useCallback, useState } from "react";
import { ArrowLeft, ArrowRight } from "lucide-react";

import type { Cell, View } from "@/lib/dashboard";
import { canonicalView } from "@/lib/dashboard";
import { defaultCell } from "@/lib/panel-kit/defaultCell";
import { SOURCELESS_VIEWS } from "@/lib/panel-kit";
import { cellToEditorState, editorStateToCell, type EditorState } from "@/lib/panel-kit/cellEditorState";
import { defaultOptionsForView } from "@/features/panel-builder/viewOptions";
import { ResultRowsProvider } from "@/features/panel-builder/fields/RowsContext";
import { usePanelData } from "@/features/dashboard/builder/usePanelData";
import { Button } from "@/components/ui/button";

import { SourceStep } from "./SourceStep";
import { ChartTypeStep } from "./ChartTypeStep";
import { OptionsStep } from "./OptionsStep";
import { TransformStep } from "./TransformStep";
import { WizardPreview } from "./WizardPreview";
import { useSplitPane } from "./useSplitPane";
import { useWizardPreview, WIZARD_CELL_I } from "./useWizardPreview";
import { WIZARD_STEPS, type WizardStepId } from "./steps";

/** A prefilled federation source + query (the Datasources page's "Create panel" entry) — seeds the
 *  wizard's first step so the user lands already bound to the query they just ran. */
export interface PanelPrefill {
  /** The federation datasource name. */
  source: string;
  /** The SQL that ran (the real statement — compiled SQL for PRQL). */
  sql: string;
}

interface Props {
  /** The viewer's session workspace. */
  ws: string;
  /** The dashboard id the finished panel will be saved into (step 8). In PICK mode this is the
   *  `PICK_DASHBOARD` sentinel — the real target is chosen on the Save step. */
  dashboardId: string;
  /** Navigate back to the dashboard. `landOn` (PICK mode, after Save) is the id the panel was saved
   *  into, so the caller lands on that dashboard rather than the sentinel. */
  onExit: (landOn?: string) => void;
  /** PICK mode: no destination dashboard was chosen up front (the Datasources "Create panel" entry).
   *  The wizard renders a "Save into which dashboard?" picker on its Save step and gates Save on it. */
  pickDashboard?: boolean;
  /** A prefilled datasource + query (the Datasources "Create panel" entry) — seeds the source step. */
  prefill?: PanelPrefill;
  /** EDIT mode: the existing cell being edited. When set, the wizard seeds from this cell and Save
   *  REPLACES it in place (keeping its geometry + key) instead of appending a new panel. Omitted ⇒
   *  the create flow (a fresh `defaultCell` seed, append at the next free row). */
  editCell?: Cell;
}

/** Seed EditorState from a prefilled federation datasource + query — mirrors SourceStep's
 *  `selectDatasource` + `adoptSql` (a `federation.query` target, table view, the SQL in code mode) so
 *  the wizard opens already bound to the ran query. */
function seedFromPrefill(ws: string, prefill: PanelPrefill): EditorState {
  const base = cellToEditorState(
    defaultCell("table", WIZARD_CELL_I, undefined, defaultOptionsForView("table")),
  );
  return {
    ...base,
    sql: { mode: "code", rawSql: prefill.sql, format: "table" },
    targets: [
      {
        refId: "A",
        tool: "federation.query",
        args: { source: prefill.source, sql: prefill.sql },
        datasource: { type: "federation", uid: `datasource:${ws}:${prefill.source}` },
      },
    ],
  };
}

/** Reset the per-view `options` to the new view's defaults when the chart type changes (mirrors the
 *  editor's switch-viz behavior — only per-view options reset; targets + fieldConfig carry over). */
function withViewReset(next: View): Partial<EditorState> {
  return { view: next, options: defaultOptionsForView(next) };
}

export function PanelWizard({ ws, dashboardId, onExit, pickDashboard, prefill, editCell }: Props) {
  // The wizard's working state. In EDIT mode it seeds from the existing cell (the SAME
  // `cellToEditorState` the editor uses — no wizard-only field, so no drift); with a `prefill` (the
  // Datasources "Create panel" entry) it seeds bound to the ran query; in CREATE mode it seeds from
  // `defaultCell(...)`, the same seed ADD uses. Every step writes through `patch`.
  const [state, setState] = useState<EditorState>(() =>
    editCell
      ? cellToEditorState(editCell)
      : prefill
        ? seedFromPrefill(ws, prefill)
        : cellToEditorState(defaultCell("timeseries", WIZARD_CELL_I, undefined, defaultOptionsForView("timeseries"))),
  );
  // PICK mode (no dashboard chosen up front): the destination dashboard the user selects on the Save
  // step. Save is gated on it. Non-pick mode always targets `dashboardId`.
  const [pickedDashboard, setPickedDashboard] = useState<string>("");
  const [step, setStep] = useState<WizardStepId>("source");
  const [frozen, setFrozen] = useState(false);
  const [saving, setSaving] = useState(false);
  // The option the OptionsStep is editing (hover/focus) — points the ONE pinned OptionFocusPreview at it
  // (the scope's resolved decision #3: one configurable preview, never per-option renderers).
  const [focusedOption, setFocusedOption] = useState<string | undefined>(undefined);

  const patch = useCallback((next: Partial<EditorState>) => {
    setState((s) => ({ ...s, ...next }));
  }, []);

  /** Switch the chart type — resets per-view options, keeps the targets + fieldConfig. */
  const changeView = useCallback(
    (next: View) => {
      if (canonicalView(next) === canonicalView(state.view || "timeseries")) return;
      patch(withViewReset(next));
    },
    [patch, state.view],
  );

  // The wizard's serialized preview cell + refresh tick (useWizardPreview: the named seam over the
  // fetch/shape cost model). Presentation-option patches flow into `state`; the hook re-derives the cell;
  // useVizQuery's fetch/shape split ensures a presentation toggle reshapes cached frames (no fetch).
  const { cell, refreshKey, bump } = useWizardPreview(state);
  // The draft's resolved rows — the SAME `usePanelData` resolution the preview renders (useVizQuery's
  // fetch key dedupes; no second query). Feeds the rows context (the template step's "Copy AI prompt"
  // embeds real data) and the preview's JSON mode.
  const { rows } = usePanelData(cell, undefined, refreshKey, { frozen });
  // The draggable step↔preview split (display-only, persisted per-browser).
  const split = useSplitPane();
  const patchAndBump = useCallback(
    (next: Partial<EditorState>) => {
      patch(next);
      bump();
    },
    [patch, bump],
  );

  const stepIndex = WIZARD_STEPS.findIndex((s) => s.id === step);
  // The source step gates on a picked target — EXCEPT for a sourceless view (e.g. `insights`), which
  // reads its own data and binds no target. Such a view is chosen on the source step itself (the "no
  // data source" affordance), so once picked, Next is free.
  const sourceless = SOURCELESS_VIEWS.has(canonicalView(state.view || "timeseries"));
  const canAdvance = step === "source" ? sourceless || !!state.targets[0]?.tool : true;

  /** Save the finished panel: serialize the wizard's state through `editorStateToCell` (the SAME path the
   *  editor's Save uses), append to the dashboard's cells, persist via `dashboard.save`. The host re-checks
   *  `mcp:dashboard.save:call` on save (the wizard's only cap; no new verb). */
  // The dashboard the panel lands in: the picked one in PICK mode, else the route's `dashboardId`.
  const targetDashboardId = pickDashboard ? pickedDashboard : dashboardId;

  const save = useCallback(async () => {
    if (!targetDashboardId) return; // PICK mode with no dashboard chosen — the button is disabled.
    setSaving(true);
    try {
      // In EDIT mode the base IS the existing cell, so serializing keeps its key + geometry (the
      // fields `editorStateToCell` doesn't own). In CREATE mode the base is a fresh `defaultCell`.
      const draftBase = editCell ?? defaultCell(state.view || "timeseries", WIZARD_CELL_I);
      const cell = editorStateToCell(state, draftBase);
      const { getDashboard, saveDashboard } = await import("@/lib/dashboard/dashboard.api");
      const target = await getDashboard(targetDashboardId);
      let cells: Cell[];
      if (editCell) {
        // REPLACE the edited cell in place — same key, same geometry (carried by `draftBase`).
        cells = target.cells.map((c) => (c.i === editCell.i ? { ...cell, i: editCell.i } : c));
      } else {
        // Place at the next free y so the new panel doesn't overlap (mirrors the editor's append).
        const maxY = target.cells.reduce((m, c) => Math.max(m, c.y + c.h), 0);
        cells = [...target.cells, { ...cell, i: `panel-${Date.now()}`, x: 0, y: maxY }];
      }
      await saveDashboard(targetDashboardId, target.title, cells, target.variables ?? []);
      onExit(targetDashboardId);
    } finally {
      setSaving(false);
    }
  }, [state, targetDashboardId, onExit, editCell]);

  return (
    <div className="flex h-full flex-col" aria-label="panel wizard" data-wizard-step={step}>
      <header className="flex items-center justify-between border-b border-border px-4 py-2">
        <div className="grid gap-0.5">
          <div className="text-sm font-medium text-fg">{editCell ? "Edit panel" : "New panel"}</div>
          <div className="text-[11px] text-muted">
            {pickDashboard ? "choose a dashboard on the last step" : `dashboard ${dashboardId}`}
          </div>
        </div>
        <Button variant="ghost" size="sm" onClick={() => onExit()} aria-label="cancel wizard">
          Cancel
        </Button>
      </header>

      {/* The step nav — addressable, not forced. A user who knows what they want can jump. */}
      <nav className="flex items-center gap-1 border-b border-border px-4 py-1.5" aria-label="wizard steps">
        {WIZARD_STEPS.map((s, i) => {
          const reached = i <= stepIndex || (s.id === "options" && !!state.targets[0]?.tool && !!state.view);
          const active = s.id === step;
          return (
            <Button
              key={s.id}
              variant={active ? "default" : "ghost"}
              size="sm"
              disabled={!reached}
              aria-current={active ? "step" : undefined}
              onClick={() => reached && setStep(s.id)}
              className="h-auto px-2.5 py-1 text-xs"
            >
              {i + 1}. {s.label}
            </Button>
          );
        })}
      </nav>

      <div
        ref={split.containerRef}
        className="grid min-h-0 flex-1 p-4"
        style={{ gridTemplateColumns: `minmax(0,${split.fraction}fr) auto minmax(0,${1 - split.fraction}fr)` }}
      >
        <ResultRowsProvider rows={rows}>
        <div className="flex min-h-0 flex-col gap-3 overflow-y-auto pr-1" aria-label="wizard step">
          {step === "source" && (
            <SourceStep
              ws={ws}
              state={state}
              patch={patchAndBump}
              onPickView={changeView}
              onAdvance={() => setStep(WIZARD_STEPS[Math.min(stepIndex + 1, WIZARD_STEPS.length - 1)]!.id)}
            />
          )}
          {step === "chartType" && (
            <ChartTypeStep
              state={state}
              onChange={changeView}
              draft={cell}
              patch={patch}
              refreshKey={refreshKey}
              ws={ws}
            />
          )}
          {step === "options" && (
            <OptionsStep
              state={state}
              patch={patch}
              onFocusOption={setFocusedOption}
              focusedOption={focusedOption}
            />
          )}
          {step === "transform" && (
            <TransformStep
              state={state}
              patch={patchAndBump}
              cell={cell}
              refreshKey={refreshKey}
              frozen={frozen}
              onFrozenChange={setFrozen}
              onSave={save}
              saving={saving}
              pickDashboard={pickDashboard}
              selectedDashboard={pickedDashboard}
              onSelectDashboard={setPickedDashboard}
              canSave={!!targetDashboardId}
            />
          )}

          <div className="mt-2 flex items-center justify-between">
            <Button
              variant="ghost"
              size="sm"
              disabled={stepIndex === 0}
              onClick={() => stepIndex > 0 && setStep(WIZARD_STEPS[stepIndex - 1]!.id)}
            >
              <ArrowLeft size={12} className="mr-1" /> Back
            </Button>
            {step !== "transform" && (
              <Button
                size="sm"
                disabled={!canAdvance}
                onClick={() => stepIndex < WIZARD_STEPS.length - 1 && setStep(WIZARD_STEPS[stepIndex + 1]!.id)}
              >
                Next <ArrowRight size={12} className="ml-1" />
              </Button>
            )}
          </div>
        </div>

        {/* The draggable divider — resize the step↔preview balance with the mouse or arrow keys. */}
        <div
          role="separator"
          aria-orientation="vertical"
          aria-label="resize wizard panes"
          tabIndex={0}
          onPointerDown={split.onPointerDown}
          onKeyDown={split.onKeyDown}
          className="group mx-1.5 flex w-2 cursor-col-resize touch-none items-center justify-center rounded-sm outline-none focus-visible:ring-1 focus-visible:ring-accent"
        >
          <div className="h-10 w-0.5 rounded-full bg-border transition-colors group-hover:bg-accent/60 group-focus-visible:bg-accent" />
        </div>

        {/* The pinned full-panel preview — the ONE chart in the wizard, with a display-only
            Chart | Table | JSON toggle. On the options step, chart mode renders through
            OptionFocusPreview so the focused option's region is emphasized (same WidgetView, one render
            path — no drift). The freeze toggle (TransformStep) pins the FETCH so transform edits reshape
            frames, no re-fetch. */}
        <div className="min-h-0">
          <WizardPreview
            cell={cell}
            ws={ws}
            refreshKey={refreshKey}
            frozen={frozen}
            step={step}
            focusedOption={step === "options" ? focusedOption : undefined}
            rows={rows}
          />
        </div>
        </ResultRowsProvider>
      </div>
    </div>
  );
}
