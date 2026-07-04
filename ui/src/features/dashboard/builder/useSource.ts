// The v2 source hook — runs a cell's `{ tool, args }` source through the WidgetBridge and shapes the
// result into rows + a latest value (widget-builder scope, "Replace the data layer"). This is the
// generalization of `useSeries`: where v1 read only `series.*`, v2 reads ANY granted read tool and
// streams a `series.watch`/`bus.watch` source over the SSE via `bridge.watch`. A denied/empty source
// degrades to an honest `denied`/empty state — never a fake value (the no-mock rule, UI too).
//
// The result shape is introspected (rubix-cube's `transformDataToColumns` analog): a tool may return
// `{ samples: [...] }`, a bare array, a `{ value }` scalar, or `{ ok }` — we normalize to `rows` (for
// chart/table/plot/d3/template) and `latest` (for stat/gauge/control read).

import { useEffect, useMemo, useRef, useState } from "react";

import type { Source } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { interpolateArgs, emptyScope } from "@/lib/vars";
import { makeWidgetBridge } from "./widgetBridge";

/** A normalized source result. `rows` for tabular/charted views; `latest` for scalar views.
 *
 *  `meta` is OPTIONAL query telemetry for the editor's status bar (data-studio-ux scope) — frame count,
 *  fetch duration, the human error text, and whether the last result came from cached raw frames (a
 *  shape-only edit) vs a fresh datasource fetch. Renderers ignore it; only the status bar reads it, so
 *  adding it breaks no view. */
export interface SourceState {
  rows: Array<Record<string, unknown>>;
  latest: unknown;
  loading: boolean;
  denied: boolean;
  meta?: QueryMeta;
}

/** Editor status-bar telemetry for one panel-data resolution. All fields optional — a live/flow path
 *  fills only what it knows. */
export interface QueryMeta {
  /** Number of canonical frames returned (viz.query path). */
  frames?: number;
  /** Wall-clock of the last fetch/shape round-trip, ms (client-measured). */
  ms?: number;
  /** The gateway/tool error text when `denied` — shown inline instead of a silent empty chart. */
  error?: string;
  /** How the current rows were produced: a fresh datasource fetch, a shape-only pass over cached raw
   *  frames (no datasource hit), a live stream, or a flow read. Drives the status bar's provenance line. */
  source?: "fetch" | "shaped" | "live" | "flow";
  /** Epoch ms the underlying RAW frames were fetched (so "as of …" reflects the data, not a reshape). */
  fetchedAt?: number;
  /** The data inspector's payload (data-studio-ux): the RESOLVED request that ran (post-interpolation
   *  targets/SQL — what the user should read to debug), the raw + shaped frames, and the effective rows.
   *  Populated only on the viz.query path; the inspector drawer renders it. Kept behind `inspect` so the
   *  status bar's hot path doesn't carry the frame arrays around. */
  inspect?: InspectPayload;
}

/** What the data-inspector drawer shows for one resolution. */
export interface InspectPayload {
  /** The resolved request the fetch sent — interpolated `sources[]`/`source` (the SQL/tool+args that
   *  actually ran), so the author reads the real query, not the pre-interpolation template. */
  request?: unknown;
  /** The raw frames the datasource returned (pre-pipeline). */
  rawFrames?: unknown;
  /** The shaped frames after the transform/field-config pipeline (absent when there is no pipeline). */
  shapedFrames?: unknown;
}

const BACKFILL = 200;

/** Normalize a tool result into rows. Handles `{samples}`, a bare array, a single object, or a scalar. */
function toRows(result: unknown): Array<Record<string, unknown>> {
  if (result == null) return [];
  if (Array.isArray(result)) return result as Array<Record<string, unknown>>;
  if (typeof result === "object") {
    const o = result as Record<string, unknown>;
    // `reminders` unwraps `reminder.list` → `{reminders:[…]}` into N rows (the channel-rich-responses
    // reminders tenant); keep in lock-step with the host mirror `viz/frame.rs::ROW_KEYS`.
    for (const k of ["samples", "items", "rows", "templates", "dashboards", "reminders"]) {
      if (Array.isArray(o[k])) return o[k] as Array<Record<string, unknown>>;
    }
    return [o];
  }
  return [{ value: result }];
}

/** Pull a scalar "latest" value from a result (the newest row's `value`/`payload`, or the scalar). */
function toLatest(rows: Array<Record<string, unknown>>, result: unknown): unknown {
  if (rows.length) {
    const last = rows[rows.length - 1];
    return last.value ?? last.payload ?? last;
  }
  if (result && typeof result === "object") {
    const o = result as Record<string, unknown>;
    return o.value ?? o.latest ?? o.payload ?? null;
  }
  return result ?? null;
}

/** Run `source` through a bridge bound to `tools` (the cell's tool set ∩ grant). Streams when the
 *  source tool is a watch verb. Re-runs when the source, tool set, or variable scope changes.
 *
 *  Slice 3: `source.args` is interpolated against `scope` (the resolved VarScope) BEFORE the bridge call,
 *  so a cell re-points by variable. For a `store.query` source, interpolation runs into the bound `vars`
 *  (and only safe arg leaves) — never string-spliced SQL; the host's parse-allowlist stays the boundary. */
export function useSource(
  source: Source | undefined,
  tools: string[],
  scope: VarScope = emptyScope(),
  refreshKey = 0,
): SourceState {
  const [state, setState] = useState<SourceState>({
    rows: [],
    latest: null,
    loading: true,
    denied: false,
  });
  const toolsKey = tools.join("|");
  // eslint-disable-next-line react-hooks/exhaustive-deps -- re-create the bridge only when the tool SET changes
  const bridge = useMemo(() => makeWidgetBridge(tools), [toolsKey]);
  // Interpolate the args against the variable scope BEFORE the call (the Slice-3 payoff). Re-key on the
  // interpolated args so a variable change (selection/refresh) re-runs the source.
  const args = useMemo(
    () => interpolateArgs(source?.args ?? {}, scope) as Record<string, unknown>,
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [JSON.stringify(source?.args ?? null), JSON.stringify(scope)],
  );
  // `refreshKey` bumps on an auto-refresh tick (Slice 4) so a non-watch read re-runs (polls state).
  const key = `${source?.tool ?? ""}:${JSON.stringify(args)}:${refreshKey}`;
  const unwatchRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    let cancelled = false;
    unwatchRef.current?.();
    unwatchRef.current = null;
    if (!source?.tool) {
      setState({ rows: [], latest: null, loading: false, denied: true });
      return;
    }
    setState((s) => ({ ...s, loading: true, denied: false }));

    (async () => {
      // A watch verb streams; for the initial paint we still try a one-shot read of the same series via
      // a `series.read`-shaped sibling is the builder's job — here we just open the stream and fold.
      const isWatch = source.tool === "series.watch" || source.tool === "bus.watch";
      if (!isWatch) {
        try {
          const result = await bridge.call(source.tool, args);
          if (cancelled) return;
          const rows = toRows(result).slice(-BACKFILL);
          setState({ rows, latest: toLatest(rows, result), loading: false, denied: false });
        } catch {
          if (cancelled) return;
          setState({ rows: [], latest: null, loading: false, denied: true });
        }
        return;
      }

      // Streaming source: start empty-but-not-denied, fold each event into the tail.
      setState({ rows: [], latest: null, loading: false, denied: false });
      unwatchRef.current = bridge.watch(source.tool, args, (event) => {
        if (cancelled) return;
        setState((s) => {
          const row = (event as Record<string, unknown>) ?? {};
          const rows = [...s.rows, row].slice(-BACKFILL);
          return { ...s, rows, latest: toLatest(rows, event) };
        });
      });
    })();

    return () => {
      cancelled = true;
      unwatchRef.current?.();
      unwatchRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `key` already encodes source.tool + interpolated args
  }, [key, bridge]);

  return state;
}
