// The wizard's preview-per-option renderer (panel-wizard scope, step 2). ONE component that wraps the
// REAL `WidgetView` (the dashboard's render dispatch) and accepts an `optionFocus` prop naming the option
// whose effect the wizard is currently isolating. The render is byte-identical to the dashboard's — there
// is no second renderer, so wizard and dashboard can never drift (the load-bearing rule). `optionFocus`
// only adds an outer `data-option-focus` attribute + a focus-region class; co-located CSS uses that to
// visually emphasize the region the option affects (the value readout for `decimals`/`unit`/`thresholds`/
// `color`; the chart canvas for `custom.*` graph styles). The emphasis is a PRESENTATION concern — the
// rendered output is the truth, the wrapper just draws the eye.
//
// Why ONE component and not per-option `<DecimalsPreview>`/`<ThresholdsPreview>`/… : N renderers drift
// from the real panel render — the exact disease this scope cures. The real `WidgetView` IS the preview;
// we never re-implement a chart or a value readout here. One render path = no drift.
//
// One responsibility: render the cell through `WidgetView`, surfaced with a focus marker.

import type { Cell } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";
import type { ExtRow } from "@/lib/ext/ext.api";
import { WidgetView } from "@/features/dashboard/views/WidgetView";
import "./optionFocusPreview.css";

/** Options whose visible effect lands on the value readout (text/color). */
const VALUE_REGION_OPTIONS = new Set([
  "displayName",
  "unit",
  "decimals",
  "min",
  "max",
  "noValue",
  "color",
  "thresholds",
  "mappings",
  "links",
]);

/** Options whose visible effect lands on the chart canvas (draw style / line / region). */
function isChartRegion(optionId: string): boolean {
  return optionId.startsWith("custom.");
}

/** The focus-region class for `optionId` — drives the co-located CSS's emphasis. `undefined` when the
 *  option has no specific region (the preview renders without zoom). */
function focusRegionFor(optionId: string | undefined): string | undefined {
  if (!optionId) return undefined;
  if (VALUE_REGION_OPTIONS.has(optionId)) return "focus-region-value";
  if (isChartRegion(optionId)) return "focus-region-chart";
  return undefined;
}

interface Props {
  /** The cell whose options are being previewed. This is the wizard's working `EditorState` serialized
   *  back to a cell via `editorStateToCell` — the same shape `dashboard.save` will persist. */
  cell: Cell;
  /** The viewer's session workspace — threaded to `WidgetView` for ext tiles + the resolved scope. */
  workspace: string;
  /** The option whose effect the wizard is currently isolating. Absent ⇒ the full-panel preview (no
   *  emphasis); present ⇒ the wrapper tags the region the option affects for the CSS to emphasize. */
  optionFocus?: { optionId: string };
  /** Installed extensions (from `ext.list`) — forwarded to `WidgetView` so an `ext:` cell mounts. */
  installed?: ExtRow[];
  /** The resolved variable scope (forwarded to `WidgetView`). */
  scope?: VarScope;
  /** Auto-refresh tick (forwarded to `WidgetView`). */
  refreshKey?: number;
}

/** Render `cell` through the real `WidgetView`, surfaced with an `optionFocus` marker the co-located CSS
 *  uses to emphasize the region the focused option affects. One render path = no drift from the dashboard. */
export function OptionFocusPreview({
  cell,
  workspace,
  optionFocus,
  installed = [],
  scope = emptyScope(),
  refreshKey = 0,
}: Props) {
  const region = focusRegionFor(optionFocus?.optionId);
  const className = region ? `option-focus-preview ${region}` : "option-focus-preview";
  return (
    <div
      className={className}
      data-option-focus={optionFocus?.optionId ?? ""}
      aria-label="option focus preview"
    >
      <WidgetView cell={cell} workspace={workspace} installed={installed} scope={scope} refreshKey={refreshKey} />
    </div>
  );
}
