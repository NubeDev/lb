// ChartTypeStep (panel-wizard scope, step 4) — the wizard's second step: pick a chart type + do the
// BASIC visual setup for it, right where it was chosen. Reuses the SHIPPED editor surfaces (no second
// authoring surface, no drift):
//   - `VizPicker` — the same shape-validated picker the editor's panel-options tab mounts;
//   - plottable views mount the editor's `PlotAxesTab` (the shared `PlotBuilder`) with `preview=false`
//     — the wizard's ONE pinned preview beside the steps is the chart; no duplicate mini-chart here;
//   - `template` mounts the editor's `TemplateOptionsEditor` (CodeMirror HTML body + the "Copy AI
//     prompt" copier over the draft's real rows);
//   - `genui` mounts the editor's `GenUiAuthorTab` (prompt → agent → accept).
// Everything else (thresholds, mappings, legend…) is the Options step — the ADVANCED settings.
// One responsibility: pick a chart type + its basic per-view setup.

import type { Cell, View } from "@/lib/dashboard";
import type { EditorState } from "@/lib/panel-kit/cellEditorState";
import { canonicalView } from "@/lib/dashboard";
import { PLOTTABLE_VIEWS } from "@/lib/panel-kit";
import { VizPicker } from "@/features/panel-builder/VizPicker";
import { PlotAxesTab } from "@/features/panel-builder/tabs/PlotAxesTab";
import { TemplateOptionsEditor } from "@/features/panel-builder/tabs/options/TemplateOptionsEditor";
import { GenUiAuthorTab } from "@/features/panel-builder/tabs/GenUiAuthorTab";
import { StatBasics } from "./StatBasics";

interface Props {
  state: EditorState;
  onChange: (view: View) => void;
  /** The wizard's serialized draft cell — supplies the plot editor's query fields. */
  draft: Cell;
  /** Writes `options.plot` / template body / genui spec without a view reset. */
  patch: (next: Partial<EditorState>) => void;
  refreshKey: number;
  /** The viewer's session workspace — the genui author tab invokes the agent with it. */
  ws: string;
}

export function ChartTypeStep({ state, onChange, draft, patch, refreshKey, ws }: Props) {
  const current = canonicalView((state.view || "timeseries") as View);
  const canPlot = PLOTTABLE_VIEWS.has(current);
  return (
    <div className="grid gap-4" aria-label="wizard chart-type step">
      <div className="grid gap-1">
        <h2 className="text-sm font-medium text-fg">Pick a chart type</h2>
        <p className="text-xs text-muted">
          The picker disables views the current data shape can&rsquo;t honestly fill.
        </p>
      </div>
      <VizPicker view={current} onChange={onChange} />
      <p className="text-[11px] text-muted" aria-label="wizard view picked">
        current: <code className="text-fg">{current}</code>
      </p>

      {canPlot && (
        <section className="grid gap-2 border-t border-border pt-4" aria-label="wizard plot basics">
          <div className="grid gap-1">
            <h2 className="text-sm font-medium text-fg">Plot</h2>
            <p className="text-xs text-muted">
              The basics — chart style and which fields go where. The preview on the right updates live;
              fine-tune everything else in <span className="text-fg">3. Options</span>.
            </p>
          </div>
          <PlotAxesTab draft={draft} state={state} patch={patch} refreshKey={refreshKey} preview={false} />
        </section>
      )}

      {current === "stat" && (
        <section className="grid gap-2 border-t border-border pt-4" aria-label="wizard stat section">
          <div className="grid gap-1">
            <h2 className="text-sm font-medium text-fg">Stat</h2>
            <p className="text-xs text-muted">
              The basics — sparkline, thresholds, and value mappings. The preview on the right updates
              live; fine-tune everything else in <span className="text-fg">3. Options</span>.
            </p>
          </div>
          <StatBasics state={state} patch={patch} />
        </section>
      )}

      {current === "template" && (
        <section className="grid gap-2 border-t border-border pt-4" aria-label="wizard template body">
          <div className="grid gap-1">
            <h2 className="text-sm font-medium text-fg">Template body</h2>
            <p className="text-xs text-muted">
              Author the HTML the panel renders — or copy the AI prompt (it embeds this draft&rsquo;s real
              rows) and paste the reply back into the editor. The preview on the right is the real render.
            </p>
          </div>
          <TemplateOptionsEditor state={state} patch={patch} />
        </section>
      )}

      {current === "genui" && (
        <section className="grid gap-2 border-t border-border pt-4" aria-label="wizard ai widget author">
          <div className="grid gap-1">
            <h2 className="text-sm font-medium text-fg">AI widget</h2>
            <p className="text-xs text-muted">
              Describe the widget; the agent authors it against this draft&rsquo;s data.
            </p>
          </div>
          <GenUiAuthorTab state={state} patch={patch} ws={ws} />
        </section>
      )}
    </div>
  );
}
