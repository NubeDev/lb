// The rich-result adapter (channel rich responses scope) — read a `rich_result` render-envelope and
// mount it via the SHIPPED dashboard `WidgetView`. This is THIN by design: it builds a v2 `Cell` from
// the envelope and hands it to the one render dispatcher; it is NOT a second render system (no renderer,
// no trust router, no bridge live here — WidgetView owns all three). One responsibility: turn a
// RichResultPayload into a mounted widget.
//
// The leash: the Cell's `source` + `action` tools drive `cellTools(cell)` (the bridge's forwardable
// set); we fold the envelope's declared `tools` in so the bridge set = render.tools. `render.tools` is
// the DECLARED set — the host intersects it with the viewer's install grant server-side (render.tools ∩
// grant) on every bridged call, so a response naming a tool the viewer wasn't granted is denied there
// regardless of what reached the bridge.
//
// Data path: the shipped read views load rows through a `source` (usePanelData → the bridge). A v2
// version we understand renders; a `v` NEWER than 2 degrades to an honest note (fallback AT render, not
// parse). Inline `data` with no source has no shipped read path (the views are source-backed) — the
// Phase-1 responses are source-backed (reminder.list re-runs `reminder.list`), so we do not build a new
// inline path; a data-only envelope with no source degrades honestly.
//
// Per-row controls: a `table` whose `options.rowControls` is set renders through {@link ResponseTable}
// (the one interactive-list piece — the shipped TablePanel has no per-row control column), which reuses
// the shipped SwitchControl/ButtonControl per row with the row object as the control's VarScope. Every
// other view mounts straight through WidgetView.

import type { RichResultPayload } from "@/lib/channel/payload.types";
import type { Cell, View, Source, Action } from "@/lib/dashboard";
import type { ExtRow } from "@/lib/ext/ext.api";
import { emptyScope } from "@/lib/vars";
import { WidgetView } from "@/features/dashboard/views/WidgetView";
import { ResponseTable, type RowControl } from "./ResponseTable";

/** The version of the render envelope this UI understands. A `v` above it degrades at render. */
const UNDERSTOOD_V = 2;

interface Props {
  payload: RichResultPayload;
  /** The viewer's session workspace — WidgetView threads it to an ext tile / the resolved scope. The
   *  host re-derives the real workspace from the token per call, so this is not a trust boundary. */
  workspace: string;
  /** Installed extensions (from `ext.list`) — threaded to WidgetView so an `ext:<id>/<widget>` RESPONSE
   *  view can mount the extension's real tile. Absent → no ext tile resolves (renders "not installed"). */
  installed?: ExtRow[];
  /** A stable key for the built cell (the channel item id) — keeps react-grid-layout/react keys stable. */
  itemKey?: string;
}

/** Read `options.rowControls` off the envelope (the table's per-row control column). Absent/typed wrong
 *  → no row controls (the table renders read-only through the shipped path). */
function rowControlsOf(options: Record<string, unknown> | undefined): RowControl[] | null {
  const rc = options?.rowControls;
  if (!Array.isArray(rc) || rc.length === 0) return null;
  return rc as RowControl[];
}

/** Build a v2 `Cell` from the render envelope. `binding` is required by the v2 `Cell` type but unused
 *  for a v2 tool-bound cell — a harmless `{ series: "" }` placeholder, exactly as other v2 cells set it
 *  (see usePanelData's EMPTY_PANEL). The tool set is folded so `cellTools(cell)` covers every declared
 *  tool: the source/action tools land on the cell, and the envelope's extra `tools` (row-control write
 *  verbs) ride on a placeholder `sources[]` so the bridge leash = render.tools. */
export function buildCell(payload: RichResultPayload, itemKey: string): Cell {
  const source = payload.source as Source | undefined;
  const action = payload.action as Action | undefined;
  // Fold the declared `tools` into the cell's forwardable set. `cellTools` reads source/action/sources
  // tools; a row-control write verb (e.g. `reminder.update`) is neither the read source nor the single
  // control action, so we carry the remaining declared tools as hidden extra targets — they widen the
  // leash to render.tools without adding a real read (the host re-checks grant regardless).
  const extraTools = (payload.tools ?? []).filter(
    (t) => t !== source?.tool && t !== action?.tool,
  );
  return {
    i: itemKey,
    x: 0,
    y: 0,
    w: 12,
    h: 8,
    v: 2,
    widget_type: "chart",
    view: payload.view as View,
    binding: { series: "" },
    source,
    action,
    sources: extraTools.map((tool, idx) => ({
      refId: `T${idx}`,
      tool,
      datasource: { type: "surreal" as const },
      hide: true,
    })),
    options: payload.options,
  };
}

export function ResponseView({ payload, workspace, installed = [], itemKey = "rich" }: Props) {
  // Version gate: degrade a newer envelope honestly (fallback AT render — parsePayload still parsed it).
  if (payload.v > UNDERSTOOD_V) {
    return (
      <div className="rounded-md border border-border bg-panel px-3 py-2 text-xs text-muted" role="status">
        this response needs a newer app to display (v{payload.v})
      </div>
    );
  }

  const cell = buildCell(payload, itemKey);
  const rowControls = payload.view === "table" ? rowControlsOf(payload.options) : null;

  // A table with per-row controls is the interactive-list case — the shipped TablePanel has no control
  // column, so render through the thin ResponseTable (reuses usePanelData + SwitchControl/ButtonControl).
  if (rowControls) {
    return <ResponseTable cell={cell} rowControls={rowControls} />;
  }

  // Every other view (and a read-only table) mounts straight through the one shipped dispatcher. Thread
  // `installed` so an `ext:<id>/<widget>` response view mounts the extension's real tile.
  return <WidgetView cell={cell} installed={installed} workspace={workspace} scope={emptyScope()} />;
}
