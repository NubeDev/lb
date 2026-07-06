// Per-datasource run history (query-workbench) — PURE localStorage fold: the last N UNIQUE SQL
// strings the user ran against a source, most-recent first. Restore is a client-side convenience
// (the SQL still re-runs through the host's parse gate + workspace wall); nothing here is durable
// platform state — saved queries remain the explicit `query.save` flow. Keyed by (workspace,
// source) so histories never bleed across tenants or sources on a shared browser profile.

/** One remembered run. */
export interface RunHistoryEntry {
  sql: string;
  /** Epoch ms of the (latest) run of this SQL. */
  ts: number;
}

/** The cap — "the last 10 runs (unique)". */
export const HISTORY_CAP = 10;

function key(ws: string, source: string): string {
  return `lb.query-history.${ws}.${source}`;
}

/** Load the history for (ws, source) — `[]` on missing/malformed/unavailable storage. */
export function loadHistory(ws: string, source: string): RunHistoryEntry[] {
  try {
    const raw = localStorage.getItem(key(ws, source));
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed
      .filter(
        (e): e is RunHistoryEntry =>
          typeof e === "object" && e !== null &&
          typeof (e as RunHistoryEntry).sql === "string" &&
          typeof (e as RunHistoryEntry).ts === "number",
      )
      .slice(0, HISTORY_CAP);
  } catch {
    return []; // storage denied / corrupt JSON — honest empty, never a crash
  }
}

/** Record a run: dedupe by exact SQL (a re-run moves to the front with a fresh ts), cap at
 *  `HISTORY_CAP`, persist, and return the new list (for the caller's state). */
export function recordRun(ws: string, source: string, sql: string, ts: number): RunHistoryEntry[] {
  const trimmed = sql.trim();
  if (!trimmed) return loadHistory(ws, source);
  const rest = loadHistory(ws, source).filter((e) => e.sql !== trimmed);
  const next = [{ sql: trimmed, ts }, ...rest].slice(0, HISTORY_CAP);
  try {
    localStorage.setItem(key(ws, source), JSON.stringify(next));
  } catch {
    // quota/denied — the in-memory list still serves this session
  }
  return next;
}
