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
import { makeWidgetBridge } from "./widgetBridge";

/** A normalized source result. `rows` for tabular/charted views; `latest` for scalar views. */
export interface SourceState {
  rows: Array<Record<string, unknown>>;
  latest: unknown;
  loading: boolean;
  denied: boolean;
}

const BACKFILL = 200;

/** Normalize a tool result into rows. Handles `{samples}`, a bare array, a single object, or a scalar. */
function toRows(result: unknown): Array<Record<string, unknown>> {
  if (result == null) return [];
  if (Array.isArray(result)) return result as Array<Record<string, unknown>>;
  if (typeof result === "object") {
    const o = result as Record<string, unknown>;
    for (const k of ["samples", "items", "rows", "templates", "dashboards"]) {
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
 *  source tool is a watch verb. Re-runs when the source or tool set changes. */
export function useSource(source: Source | undefined, tools: string[]): SourceState {
  const [state, setState] = useState<SourceState>({
    rows: [],
    latest: null,
    loading: true,
    denied: false,
  });
  const bridge = useMemo(() => makeWidgetBridge(tools), [tools.join("|")]);
  const key = JSON.stringify(source ?? null);
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
          const result = await bridge.call(source.tool, source.args ?? {});
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
      unwatchRef.current = bridge.watch(source.tool, source.args ?? {}, (event) => {
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
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, bridge]);

  return state;
}
