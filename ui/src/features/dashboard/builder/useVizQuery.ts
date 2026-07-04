// The Phase-3 viz.query fetch, on the dashboard READ CACHE, SPLIT into fetch vs shape (data-studio-ux:
// "edit-without-requery"). It still calls the backend host verb `viz.query` through the SAME WidgetBridge
// a cell rides (`mcp:viz.query:call`) and returns the SAME `SourceState` shape (invariant B: NO
// client-side transform execution — the pipeline stays server-side, one impl). What changed:
//
//   FETCH  — `viz.query` over the cell's `sources[]`/`source` with an EMPTY pipeline (no transformations,
//            no fieldConfig). Keyed on `vizFetchKey` = {sources, source, scope, tick} ONLY. Returns the
//            RAW frames. A transform/field-config edit does NOT re-key this, so it does NOT re-hit the
//            datasource.
//   SHAPE  — when the panel HAS a pipeline (transformations and/or fieldConfig), a COMPUTE-ONLY
//            `viz.query` (frames-in) reshapes the cached raw frames — no source resolution, no datasource
//            touch. Keyed on `vizShapeKey` = {framesHash, transformations, fieldConfig}. An option edit
//            re-keys ONLY this. No pipeline → skip the round-trip and use the raw frames directly.
//
// So switching a transform or a decimals override reshapes frames ALREADY on screen instead of re-querying
// the store — the whole point. A `frozen` panel disables the fetch entirely (keeps its last raw frames)
// so a user can iterate against an expensive query without re-running it.
//
// The three editor consumers (probe/preview/plot) that mount this for the same draft still share the cache
// entries → one fetch + one shape round-trip. The 200ms debounce sits on the KEY INPUTS. A denied/missing
// cap → the bridge throws → an honest `denied` state with the error text (CLAUDE §9).

import { useMemo, useRef } from "react";
import { useQuery } from "@tanstack/react-query";

import type { Cell, Target } from "@/lib/dashboard";
import { cellPrimaryTarget, cellSources } from "@/lib/dashboard";
import { cellTools } from "../views/WidgetView";
import { useDashboardWs } from "../cache/useDashboardWs";
import { useDebounced } from "../cache/useDebounced";
import { useFreeze } from "../cache/useFreeze";
import { vizFetchKey, vizShapeKey } from "../cache/queryKeys";
import type { VarScope } from "@/lib/vars";
import { emptyScope, interpolateArgs } from "@/lib/vars";
import { makeWidgetBridge } from "./widgetBridge";
import type { QueryMeta, SourceState } from "./useSource";

/** The qualified host verb this hook dispatches (cap `mcp:viz.query:call`). */
export const VIZ_QUERY_TOOL = "viz.query";

const DEBOUNCE_MS = 200;

/** Frames-in payload budget. Above this (serialized) the raw frames are too big to profitably POST back
 *  for a shape-only pass — re-fetching through the datasource is cheaper — so we fall back to a normal
 *  full-spec `viz.query` and mark the result `source:"fetch"`. ~4 MB is well under a request limit while
 *  covering the common editor case (a few thousand points). */
const FRAMES_IN_BUDGET_BYTES = 4_000_000;

/** A canonical column frame, as `viz.query` returns it. */
interface Frame {
  refId?: string;
  name?: string;
  fields: Array<{ name: string; type?: string; values: unknown[] }>;
  length?: number;
}

/** The `viz.query` result — canonical `frames` PLUS the primary frame flattened to `rows`. */
interface VizQueryResult {
  frames?: Frame[];
  rows?: Array<Record<string, unknown>>;
}

/** Pull a scalar "latest" from the rows (mirrors `useSource.toLatest`). */
function toLatest(rows: Array<Record<string, unknown>>): unknown {
  if (!rows.length) return null;
  const last = rows[rows.length - 1];
  return last.value ?? last.payload ?? last;
}

/** The rows a caller should render: prefer the server's primary-frame flattening (`rows`) verbatim so a
 *  responder that returns only `rows` still resolves; reconstruct from `frames[0]` only when `rows` is
 *  absent. */
function pickRows(
  serverRows: Array<Record<string, unknown>> | undefined,
  frames: Frame[] | undefined,
): Array<Record<string, unknown>> {
  if (Array.isArray(serverRows)) return serverRows;
  return framesToRows(frames);
}

/** Flatten the primary (first) frame back to rows — the SAME shape the renderers consume. */
function framesToRows(frames: Frame[] | undefined): Array<Record<string, unknown>> {
  const f = frames?.[0];
  if (!f || !Array.isArray(f.fields)) return [];
  const len = f.length ?? f.fields[0]?.values?.length ?? 0;
  const rows: Array<Record<string, unknown>> = [];
  for (let i = 0; i < len; i++) {
    const row: Record<string, unknown> = {};
    for (const field of f.fields) row[field.name] = field.values?.[i];
    rows.push(row);
  }
  return rows;
}

/** A cheap, stable hash of the raw frames so the shape key changes iff the fetched data changed. Not a
 *  cryptographic hash — a length + rolling checksum of the JSON is enough to distinguish datasets and is
 *  far cheaper than carrying the whole frame set in the key. */
function hashFrames(json: string): string {
  let h = 0;
  for (let i = 0; i < json.length; i++) h = (h * 31 + json.charCodeAt(i)) | 0;
  return `${json.length}:${h}`;
}

/** Whether the panel carries a transform/field-config pipeline worth a shape pass. No pipeline → the raw
 *  frames ARE the result and we skip the second round-trip entirely. */
function hasPipeline(panel: Cell): boolean {
  const t = (panel as { transformations?: unknown[] }).transformations;
  const fc = panel.fieldConfig;
  const hasTransforms = Array.isArray(t) && t.length > 0;
  const hasFieldConfig =
    !!fc && ((fc.overrides?.length ?? 0) > 0 || Object.keys(fc.defaults ?? {}).length > 0);
  return hasTransforms || hasFieldConfig;
}

/** Options for {@link useVizQuery}. */
export interface UseVizQueryOptions {
  /** When true, the FETCH is frozen: the datasource is not re-hit even if the source/tick changes; the
   *  last cached raw frames are reshaped instead. The editor's "use current data" toggle. */
  frozen?: boolean;
}

/** Resolve a panel's rows via the backend `viz.query` verb, SPLIT into fetch (sources → raw frames) and
 *  shape (compute-only pipeline over cached raw frames). Consumers with the same resolved spec share the
 *  cache entries; the keys are debounced so editor keystrokes fire at most one fetch + one shape. */
export function useVizQuery(
  panel: Cell,
  scope: VarScope = emptyScope(),
  refreshKey = 0,
  opts: UseVizQueryOptions = {},
): SourceState {
  const ws = useDashboardWs();
  // Explicit opt wins; otherwise inherit the ambient freeze the editor's preview subtree provides.
  const ambientFrozen = useFreeze();
  const frozen = opts.frozen ?? ambientFrozen;

  // The bridge leash = the cell's tool set ∩ grant, PLUS viz.query itself. Re-create only when it changes.
  const tools = cellTools(panel);
  const toolsKey = [...tools, VIZ_QUERY_TOOL].join("|");
  // eslint-disable-next-line react-hooks/exhaustive-deps -- re-create the bridge only when the tool SET changes
  const bridge = useMemo(() => makeWidgetBridge([...tools, VIZ_QUERY_TOOL]), [toolsKey]);

  // Interpolate each target's args against the resolved scope BEFORE the call (a `${host}` repoints the
  // series exactly as the shipped path did). The panel sent over carries the interpolated `sources[]`.
  const resolvedSources = useMemo<Target[]>(
    () =>
      cellSources(panel).map((t) => ({
        ...t,
        args: interpolateArgs(t.args ?? {}, scope) as Record<string, unknown>,
      })),
    [panel, scope],
  );
  const resolvedSource = useMemo(
    () =>
      panel.source
        ? { ...panel.source, args: interpolateArgs(panel.source.args ?? {}, scope) }
        : undefined,
    [panel.source, scope],
  );

  const hasTarget = !!cellPrimaryTarget(panel)?.tool;
  const pipeline = useMemo(
    () => ({
      transformations: (panel as { transformations?: unknown }).transformations,
      fieldConfig: panel.fieldConfig,
    }),
    [panel],
  );
  const needsShape = hasPipeline(panel);

  // ── FETCH — raw frames, EMPTY pipeline. Keyed on {sources, source, scope, tick} only. A transform/
  // field-config edit is absent from this key, so it does not re-fetch. `frozen` PINS the key: while
  // frozen we keep querying (disabled) the LAST key fetched while unfrozen, so a source edit reshapes the
  // frozen raw frames instead of pointing at a never-fetched (empty) key. Unfreeze lets the live spec flow
  // again → one fresh fetch.
  const liveFetchSpec = useMemo(
    () => ({ sources: resolvedSources, source: resolvedSource, scope, tick: refreshKey }),
    [resolvedSources, resolvedSource, scope, refreshKey],
  );
  const frozenSpecRef = useRef(liveFetchSpec);
  if (!frozen) frozenSpecRef.current = liveFetchSpec; // track the live spec whenever unfrozen
  const fetchSpec = frozen ? frozenSpecRef.current : liveFetchSpec;
  const debouncedFetchSpec = useDebounced(fetchSpec, DEBOUNCE_MS);
  const fetchQuery = useQuery({
    queryKey: vizFetchKey(ws, debouncedFetchSpec),
    enabled: hasTarget && !frozen,
    queryFn: async () => {
      const started = performance.now();
      const rawPanel = {
        ...panel,
        sources: resolvedSources,
        source: resolvedSource,
        transformations: [],
        fieldConfig: undefined,
      };
      const out = await bridge.call<VizQueryResult>(VIZ_QUERY_TOOL, { panel: rawPanel, scope });
      // Keep BOTH: `frames` feed the shape pass (frames-in); `rows` is the server's primary-frame
      // flattening, which we prefer verbatim (a responder that returns only `rows` — e.g. a thin stub or
      // a non-frame tool — still resolves; we fall back to reconstructing from frames only if absent).
      return {
        frames: out.frames ?? [],
        rows: out.rows,
        ms: performance.now() - started,
        at: Date.now(),
      };
    },
  });

  const rawFrames = fetchQuery.data?.frames;
  const framesJson = useMemo(() => (rawFrames ? JSON.stringify(rawFrames) : ""), [rawFrames]);
  const overBudget = framesJson.length > FRAMES_IN_BUDGET_BYTES;

  // ── SHAPE — compute-only pipeline over the cached raw frames. Keyed on {framesHash, pipeline}. Runs
  // ONLY when there is a pipeline, raw frames exist, and they fit the frames-in budget. Over budget → we
  // don't POST the frames back; fall through to the raw frames (the un-shaped fetch is the honest result
  // and the status bar says the pipeline was skipped for size).
  const shapeSpec = useMemo(
    () => ({
      framesHash: framesJson ? hashFrames(framesJson) : "",
      transformations: pipeline.transformations,
      fieldConfig: pipeline.fieldConfig,
    }),
    [framesJson, pipeline],
  );
  const debouncedShapeSpec = useDebounced(shapeSpec, DEBOUNCE_MS);
  const shapeEnabled = needsShape && !!rawFrames && !overBudget;
  const shapeQuery = useQuery({
    queryKey: vizShapeKey(ws, debouncedShapeSpec),
    enabled: shapeEnabled,
    queryFn: async () => {
      const started = performance.now();
      // Frames-in: NO sources, so the backend resolves nothing — it only runs the pipeline. Same verb,
      // same cap; carries the ALREADY-FETCHED raw frames.
      const shapePanel = {
        frames: rawFrames,
        transformations: pipeline.transformations ?? [],
        fieldConfig: pipeline.fieldConfig,
      };
      const out = await bridge.call<VizQueryResult>(VIZ_QUERY_TOOL, { panel: shapePanel, scope });
      return { frames: out.frames ?? [], rows: out.rows, ms: performance.now() - started };
    },
  });

  return useMemo<SourceState>(() => {
    const fetchedAt = fetchQuery.data?.at;
    const baseMeta: QueryMeta = { fetchedAt };

    // No resolvable target → honest empty (the status bar distinguishes this "never ran" case).
    if (!hasTarget) {
      return { rows: [], latest: null, loading: false, denied: true, meta: { ...baseMeta, source: "fetch" } };
    }
    // A fetch error (denied/tool error) → honest denied WITH the error text for the status bar.
    if (fetchQuery.isError) {
      const error = errText(fetchQuery.error);
      return { rows: [], latest: null, loading: false, denied: true, meta: { ...baseMeta, error, source: "fetch" } };
    }
    const fetchLoading = fetchQuery.isLoading && !frozen;

    // The pipeline path: prefer the shaped frames; while shaping (or if it errored) fall back to raw.
    if (needsShape && rawFrames) {
      if (overBudget) {
        // Too big to frames-in — show the RAW frames un-shaped and say so.
        const rows = pickRows(fetchQuery.data?.rows, rawFrames);
        return {
          rows,
          latest: toLatest(rows),
          loading: fetchLoading,
          denied: false,
          meta: { ...baseMeta, frames: rawFrames.length, ms: fetchQuery.data?.ms, source: "fetch" },
        };
      }
      if (shapeQuery.isError) {
        const error = errText(shapeQuery.error);
        return { rows: [], latest: null, loading: false, denied: true, meta: { ...baseMeta, error, source: "shaped" } };
      }
      const shaped = shapeQuery.data?.frames;
      const usingShaped = !!shaped;
      const frames = shaped ?? rawFrames;
      const rows = pickRows(usingShaped ? shapeQuery.data?.rows : fetchQuery.data?.rows, frames);
      return {
        rows,
        latest: toLatest(rows),
        loading: fetchLoading || shapeQuery.isLoading,
        denied: false,
        meta: {
          ...baseMeta,
          frames: frames.length,
          ms: (fetchQuery.data?.ms ?? 0) + (shapeQuery.data?.ms ?? 0),
          // A reshape that did NOT re-fetch (fetch settled, only the shape key moved) is the payoff.
          source: usingShaped && !fetchQuery.isFetching ? "shaped" : "fetch",
        },
      };
    }

    // No pipeline: the raw frames ARE the result — one round-trip, no shape pass.
    const rows = pickRows(fetchQuery.data?.rows, rawFrames);
    return {
      rows,
      latest: toLatest(rows),
      loading: fetchLoading,
      denied: false,
      meta: { ...baseMeta, frames: rawFrames?.length ?? 0, ms: fetchQuery.data?.ms, source: "fetch" },
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- derived purely from the query states below
  }, [
    hasTarget,
    frozen,
    needsShape,
    overBudget,
    rawFrames,
    fetchQuery.isError,
    fetchQuery.isLoading,
    fetchQuery.isFetching,
    fetchQuery.error,
    fetchQuery.data,
    shapeQuery.isError,
    shapeQuery.isLoading,
    shapeQuery.error,
    shapeQuery.data,
  ]);
}

/** Human-readable error text from a thrown bridge error (for the status bar). Denies read as "Denied". */
function errText(err: unknown): string {
  if (err instanceof Error) return err.message;
  if (typeof err === "string") return err;
  return "query failed";
}
