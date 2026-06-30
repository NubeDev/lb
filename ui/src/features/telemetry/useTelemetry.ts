// The telemetry console hook — data + state for the live console (telemetry-console scope). Owns the
// snapshot query, the live-tail SSE fold, the trace pivot, and the filter. One hook per file (the
// `.tsx` keeps the markup). Reads the REAL `telemetry.*` verbs over the real gateway; the workspace
// wall + cap gate are server-side (a deny surfaces here as an error, never fabricated rows).

import { useCallback, useEffect, useRef, useState } from "react";

import {
  openTelemetryStream,
  queryTelemetry,
  traceTelemetry,
  type TelemetryFilter,
  type TelemetryRow,
  type TelemetryStream,
} from "@/lib/telemetry";

const MAX_ROWS = 500; // the console keeps a bounded window in memory (the ring itself is capped)

export interface TelemetryState {
  rows: TelemetryRow[];
  error: string | null;
  live: boolean;
  filter: TelemetryFilter;
  /** The trace pivot: when set, `rows` shows only that correlated trace (a click on a row's traceId). */
  pivotTrace: string | null;
  setFilter: (next: TelemetryFilter) => void;
  setLive: (on: boolean) => void;
  refresh: () => Promise<void>;
  pivotToTrace: (traceId: string) => Promise<void>;
  clearPivot: () => void;
}

/** Drive the telemetry console for the session workspace. `initialFilter` seeds from the URL (so a
 *  shared link restores the view). Snapshot on filter change; live rows fold in when `live` is on. */
export function useTelemetry(initialFilter: TelemetryFilter = {}): TelemetryState {
  const [rows, setRows] = useState<TelemetryRow[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [live, setLive] = useState(false);
  const [filter, setFilter] = useState<TelemetryFilter>(initialFilter);
  const [pivotTrace, setPivotTrace] = useState<string | null>(null);
  const streamRef = useRef<TelemetryStream | null>(null);

  const refresh = useCallback(async () => {
    try {
      const page = await queryTelemetry(filter, 100);
      setRows(page.rows);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [filter]);

  // Snapshot on filter change (unless pivoted to a trace, which owns the row set).
  useEffect(() => {
    if (pivotTrace) return;
    void refresh();
  }, [refresh, pivotTrace]);

  // Live tail: open while `live` is on and not pivoted; fold each row newest-first, bounded. Client
  // filters the live row against the active filter so the toggle and the filter compose.
  useEffect(() => {
    if (!live || pivotTrace) {
      streamRef.current?.close();
      streamRef.current = null;
      return;
    }
    const stream = openTelemetryStream((row) => {
      if (!matchesFilter(row, filter)) return;
      setRows((prev) => [row, ...prev].slice(0, MAX_ROWS));
    });
    streamRef.current = stream;
    return () => {
      stream?.close();
      streamRef.current = null;
    };
  }, [live, pivotTrace, filter]);

  const pivotToTrace = useCallback(async (traceId: string) => {
    try {
      const traceRows = await traceTelemetry(traceId);
      setRows(traceRows);
      setPivotTrace(traceId);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  const clearPivot = useCallback(() => setPivotTrace(null), []);

  return {
    rows,
    error,
    live,
    filter,
    pivotTrace,
    setFilter,
    setLive,
    refresh,
    pivotToTrace,
    clearPivot,
  };
}

/** Client-side filter match for a live row (the server already applied the same filter to the
 *  snapshot; this keeps the live fold consistent with the active filter). Level is a min-severity. */
export function matchesFilter(row: TelemetryRow, f: TelemetryFilter): boolean {
  if (f.source && row.source !== f.source) return false;
  if (f.actor && row.actor !== f.actor) return false;
  if (f.outcome && row.outcome !== f.outcome) return false;
  if (f.traceId && row.traceId !== f.traceId) return false;
  if (f.text && !row.msg.toLowerCase().includes(f.text.toLowerCase())) return false;
  if (f.level && !levelAtOrAbove(row.level, f.level)) return false;
  if (f.since != null && row.ts < f.since) return false;
  if (f.until != null && row.ts >= f.until) return false;
  return true;
}

const SEVERITY = ["error", "warn", "info", "debug", "trace"];

/** True when `level` is at or above `min` in severity order (error > warn > … > trace). */
function levelAtOrAbove(level: string, min: string): boolean {
  const li = SEVERITY.indexOf(level);
  const mi = SEVERITY.indexOf(min);
  if (li < 0 || mi < 0) return true; // unknown level never filtered out
  return li <= mi;
}
