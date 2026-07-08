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
// Layout: a 2-column grid — the active step on the left, the pinned `PreviewPane` on the right. The step
// list + the dashboard id (the save target) thread through props; the wizard owns no persistence — Save
// (step 8) calls `editorStateToCell` → `dashboard.save`.
//
// One responsibility: orchestrate the wizard's EditorState + step transitions + layout.

import { useCallback, useState } from "react";
import { ArrowLeft, ArrowRight } from "lucide-react";

import type { Cell, View } from "@/lib/dashboard";
import { canonicalView } from "@/lib/dashboard";
import { defaultCell } from "@/lib/panel-kit/defaultCell";
import { cellToEditorState, editorStateToCell, type EditorState } from "@/lib/panel-kit/cellEditorState";
import { defaultOptionsForView } from "@/features/panel-builder/viewOptions";
import { PreviewPane } from "@/features/panel-builder/PreviewPane";
import { OptionFocusPreview } from "@/features/panel-builder/options/OptionFocusPreview";
import { optionById } from "@/features/panel-builder/options/registry";
import { Button } from "@/components/ui/button";

import { SourceStep } from "./SourceStep";
import { ChartTypeStep } from "./ChartTypeStep";
import { OptionsStep } from "./OptionsStep";
import { TransformStep } from "./TransformStep";
import { useWizardPreview, WIZARD_CELL_I } from "./useWizardPreview";
import { WIZARD_STEPS, type WizardStepId } from "./steps";

interface Props {
  /** The viewer's session workspace. */
  ws: string;
  /** The dashboard id the finished panel will be saved into (step 8). */
  dashboardId: string;
  /** Navigate back to the dashboard (a Cancel affordance + the post-Save redirect). */
  onExit: () => void;
}

/** Reset the per-view `options` to the new view's defaults when the chart type changes (mirrors the
 *  editor's switch-viz behavior — only per-view options reset; targets + fieldConfig carry over). */
function withViewReset(next: View): Partial<EditorState> {
  return { view: next, options: defaultOptionsForView(next) };
}

export function PanelWizard({ ws, dashboardId, onExit }: Props) {
  // The wizard's working state — `cellToEditorState(defaultCell(...))`, the SAME seed ADD uses in the
  // editor. No wizard-only field; every step writes through `patch`.
  const [state, setState] = useState<EditorState>(() =>
    cellToEditorState(defaultCell("timeseries", WIZARD_CELL_I, undefined, defaultOptionsForView("timeseries"))),
  );
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
  const patchAndBump = useCallback(
    (next: Partial<EditorState>) => {
      patch(next);
      bump();
    },
    [patch, bump],
  );

  const stepIndex = WIZARD_STEPS.findIndex((s) => s.id === step);
  const canAdvance = step === "source" ? !!state.targets[0]?.tool : true;

  /** Save the finished panel: serialize the wizard's state through `editorStateToCell` (the SAME path the
   *  editor's Save uses), append to the dashboard's cells, persist via `dashboard.save`. The host re-checks
   *  `mcp:dashboard.save:call` on save (the wizard's only cap; no new verb). */
  const save = useCallback(async () => {
    setSaving(true);
    try {
      const draftBase = defaultCell(state.view || "timeseries", WIZARD_CELL_I);
      const cell = editorStateToCell(state, draftBase);
      const { getDashboard, saveDashboard } = await import("@/lib/dashboard/dashboard.api");
      const target = await getDashboard(dashboardId);
      // Place at the next free y so the new panel doesn't overlap (mirrors the editor's append behavior).
      const maxY = target.cells.reduce((m, c) => Math.max(m, c.y + c.h), 0);
      const placed: Cell = { ...cell, i: `panel-${Date.now()}`, x: 0, y: maxY };
      await saveDashboard(dashboardId, target.title, [...target.cells, placed], target.variables ?? []);
      onExit();
    } finally {
      setSaving(false);
    }
  }, [state, dashboardId, onExit]);

  return (
    <div className="flex h-full flex-col" aria-label="panel wizard" data-wizard-step={step}>
      <header className="flex items-center justify-between border-b border-border px-4 py-2">
        <div className="grid gap-0.5">
          <div className="text-sm font-medium text-fg">New panel</div>
          <div className="text-[11px] text-muted">dashboard {dashboardId}</div>
        </div>
        <Button variant="ghost" size="sm" onClick={onExit} aria-label="cancel wizard">
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

      <div className="grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_minmax(0,1.1fr)] gap-4 p-4">
        <div className="grid min-h-0 content-start gap-3 overflow-y-auto" aria-label="wizard step">
          {step === "source" && <SourceStep ws={ws} state={state} patch={patchAndBump} />}
          {step === "chartType" && (
            <ChartTypeStep state={state} onChange={changeView} draft={cell} patch={patch} refreshKey={refreshKey} />
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

        {/* The pinned full-panel preview — the ONE chart in the wizard. On the options step it renders
            through OptionFocusPreview so the focused option's region is emphasized (same WidgetView, one
            render path — no drift). Elsewhere it is the SAME PreviewPane/WidgetView the editor uses; the
            freeze toggle (TransformStep) pins the FETCH so transform edits reshape frames, no re-fetch. */}
        <div className="min-h-0">
          {step === "options" ? (
            <div
              className="flex h-full min-h-[12rem] flex-col rounded-lg border border-border bg-panel p-3"
              aria-label="panel preview"
            >
              <div className="mb-2 flex items-center justify-between">
                <span className="text-[11px] uppercase tracking-wide text-muted">Preview</span>
                {focusedOption && (
                  <span className="rounded-sm bg-accent/10 px-1.5 py-0.5 text-[11px] text-muted" aria-label="focused option">
                    editing · {optionById(focusedOption)?.label ?? focusedOption}
                  </span>
                )}
              </div>
              <div className="min-h-0 flex-1">
                <OptionFocusPreview
                  cell={cell}
                  workspace={ws}
                  refreshKey={refreshKey}
                  optionFocus={focusedOption ? { optionId: focusedOption } : undefined}
                />
              </div>
            </div>
          ) : (
            <PreviewPane cell={cell} ws={ws} refreshKey={refreshKey} frozen={frozen} />
          )}
        </div>
      </div>
    </div>
  );
}
