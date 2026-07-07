// The query-draft streaming contract (query-draft-streaming scope) — PURE: the bus subject
// convention + the defensive frame parse. No React, no transport. An AI agent (any MCP caller with
// `mcp:bus.publish:call`) streams a query into the open workbench by publishing full
// `SqlSourceState` frames on `querybuilder/<source>/draft`; the host walls that to
// `ws/{id}/ext/querybuilder/<source>/draft`, so workspace isolation is inherited, never re-checked
// here. Full-state frames are idempotent: each one REPLACES the editor state, so reconnect/resume
// needs no history (rule 3 — the bus is motion; durable saves go through `query.save`).
//
// The parse is deliberately shallow: it proves the frame is a plausible `SqlSourceState` (an
// object with a valid `mode`, a string `rawSql`, and — when present — a `builder` with a string
// `table` and array `columns`/`filters`). Deeper invalid shapes degrade exactly like hand-typed
// bad state would: the emitters and editors are already defensive. A frame that fails the parse
// is DROPPED (returns null) — a malformed publish never crashes the editor.

import type { SqlBuilderQuery, SqlSourceState } from "@/lib/panel-kit/sql/query";

/** The workspace-relative bus subject an agent publishes query-draft frames on for `source`
 *  (`"surreal-local"` or a federation datasource name). One live draft per source per workspace
 *  (scope OQ #1 — promote to a `/draft/<id>` suffix if concurrent drafts become real). */
export function draftSubject(source: string): string {
  return `querybuilder/${source}/draft`;
}

/** True if `v` looks like a `SqlBuilderQuery` (string table + array columns/filters). */
function isPlausibleBuilder(v: unknown): v is SqlBuilderQuery {
  if (typeof v !== "object" || v === null) return false;
  const b = v as Record<string, unknown>;
  return typeof b.table === "string" && Array.isArray(b.columns) && Array.isArray(b.filters);
}

/** Parse one published frame into a `SqlSourceState`, or `null` if it isn't plausibly one.
 *  `format` defaults to `"table"` so an agent can omit it. */
export function parseDraftFrame(payload: unknown): SqlSourceState | null {
  if (typeof payload !== "object" || payload === null) return null;
  const f = payload as Record<string, unknown>;
  if (f.mode !== "builder" && f.mode !== "code") return null;
  if (typeof f.rawSql !== "string") return null;
  if (f.builder !== undefined && !isPlausibleBuilder(f.builder)) return null;
  const format = f.format === "time-series" ? "time-series" : "table";
  const state: SqlSourceState = {
    mode: f.mode,
    rawSql: f.rawSql,
    format,
  };
  if (f.builder !== undefined) state.builder = f.builder as SqlBuilderQuery;
  if (f.builderLayout !== undefined) state.builderLayout = f.builderLayout;
  return state;
}
