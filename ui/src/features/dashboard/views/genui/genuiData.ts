// The genui `/data/{refId}` data-model assembly. A genui cell's IR binds values by JSON Pointer against
// a surface data model keyed by refId (`{"$bind":"/data/A/rows"}`). Each `sources[]` Target on the cell
// is resolved through the SAME shipped panel data path (`usePanelData`) as every other view — so the
// source-picker, variables, transformations, and `viz.query`/watch cadence all apply UNCHANGED
// (genui-scope "Data targets are ordinary v3 sources[]"). This module owns only the SHAPE of the
// per-refId model + the empty-source guard reused from `WidgetView`; the hook wiring lives in the probe
// component (`GenUiView`), because React hooks can't be called from a plain function.

import type { Cell, Target } from "@/lib/dashboard";
import type { SourceState } from "../../builder/useSource";

/** The per-refId entry the IR binds into: the resolved rows/latest + the panel state flags. A binding
 *  `/data/A/rows` reads `rows`; `/data/A/value` reads the scalar/`latest`; `/data/A/latest` too. */
export interface RefData {
  rows: Array<Record<string, unknown>>;
  latest: unknown;
  /** Convenience alias so `{"$bind":"/data/A/value"}` resolves a scalar the way `stat`/`gauge` want. */
  value: unknown;
  loading: boolean;
  denied: boolean;
}

export type GenUiDataModel = { data: Record<string, RefData> };

/** Shape one target's `SourceState` into the refId entry the IR reads. */
export function refDataOf(state: SourceState): RefData {
  const value = state.latest ?? (state.rows.length === 1 ? firstScalar(state.rows[0]) : null);
  return { rows: state.rows, latest: state.latest, value, loading: state.loading, denied: state.denied };
}

/** For a single-row result, expose its first field (or the `value` field) as the scalar a stat binds. */
function firstScalar(row: Record<string, unknown>): unknown {
  if (row && typeof row === "object") {
    if ("value" in row) return row.value;
    const keys = Object.keys(row);
    if (keys.length) return row[keys[0]];
  }
  return row;
}

/** The genui cell's data targets. A genui cell always uses v3 `sources[]`; we also honour a v2 single
 *  `source` (promoted to refId `A`) so a hand-built/older cell still resolves — but skip the EMPTY
 *  placeholder `source` (`{tool:"",args:null}`) the gateway round-trips beside a real `sources[]` (the
 *  known "binding broken" trap — the same `cell.source?.tool ? … : …` guard `WidgetView` uses; NOT
 *  re-implemented divergently). */
export function genuiTargets(cell: Cell): Target[] {
  // Visible targets only — and when EVERY sources[] entry is hidden (a channel rich_result cell's
  // leash-widening extra tools, see ResponseView.buildCell), fall through to the v2 single source
  // rather than resolving nothing (channel-widgets slice: the dock preview path).
  const visible = (cell.sources ?? []).filter((t) => !t.hide);
  if (visible.length) return visible;
  if (cell.source?.tool) return [{ refId: "A", tool: cell.source.tool, args: cell.source.args }];
  return [];
}

/** A one-source synthetic cell for a single target, so `usePanelData` resolves exactly that target's
 *  data (it reads the primary target). Carries the parent's transformations/fieldConfig off — one
 *  target, one resolve. */
export function singleTargetCell(cell: Cell, target: Target): Cell {
  return {
    ...cell,
    source: { tool: "", args: undefined },
    sources: [target],
    // A per-target resolve should not re-run the whole panel's transform pipeline; the IR binds raw
    // rows. Keep it lean and predictable.
    transformations: [],
  };
}

/** True if EVERY target is denied (or there are none) — lets the view show the standard denied/empty
 *  panel state instead of an empty surface, matching every other view's deny UX. */
export function allDenied(model: GenUiDataModel, targetCount: number): boolean {
  if (targetCount === 0) return false;
  const entries = Object.values(model.data);
  return entries.length > 0 && entries.every((d) => d.denied);
}
