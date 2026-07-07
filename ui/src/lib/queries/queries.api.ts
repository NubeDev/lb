// The query API client — one call per export, mirroring the host's `query.*` MCP verbs 1:1 (query
// scope, `rust/crates/host/src/query/tool.rs`). The UI never calls `invoke` directly; it goes
// through these named verbs (FILE-LAYOUT frontend rules). Each is capability-gated server-side; the
// workspace + principal come from the session token (the hard wall, §7), never an argument.
//
// REACHED THROUGH THE MCP BRIDGE (`mcp_call` → `POST /mcp/call`). The gateway has dedicated REST
// routes for sibling CRUD surfaces (`/rules`, `/datasources`, …) but not yet for `/query`; rather
// than add gateway routes here, this client rides the universal contract the host already exposes —
// `mcp_call({tool:"query.<verb>", args})`. Rule 7 (MCP is the universal contract): the agent, the
// UI, a rule, and an extension page all reach a saved query the same way. The day dedicated
// `/query/*` routes land (mirroring `/rules/*`), only this file changes — the call sites stay.

import { invoke } from "@/lib/ipc/invoke";
import type {
  QueryCompileResult,
  QueryLang,
  QueryRunResult,
  QuerySummary,
  QueryTarget,
  SavedQuery,
} from "./queries.types";

/** Call a `query.*` MCP tool through the host-mediated bridge. The gateway re-checks
 *  `mcp:query.<verb>:call` workspace-first; a denied call throws a generic "not permitted"; author
 *  feedback (a parse error, a non-SELECT `raw`, a missing param) throws with the verbatim message. */
function mcp<T>(tool: string, args: Record<string, unknown>): Promise<T> {
  return invoke<T>("mcp_call", { tool, args });
}

/** The workspace's saved-query roster (`query.list`). Mirrors the host's flat `{queries:[...]}` —
 *  no text, no result data. A workspace without the `mcp:query.list:call` grant sees the call
 *  reject → an empty Queries group (the picker's deny-tolerant contract). */
export async function listQueries(): Promise<QuerySummary[]> {
  const res = await mcp<{ queries: QuerySummary[] }>("query.list", {});
  return res.queries;
}

/** Read one saved query by id (`query.get`). Returns the full record (for re-opening in the editor).
 *  Absent/tombstoned → NotFound (a cross-tenant id resolves to nothing). */
export function getQuery(id: string): Promise<SavedQuery> {
  return mcp<SavedQuery>("query.get", { id });
}

/** Create or update a saved query (idempotent UPSERT on `id`). Mirrors `query.save`. Returns `{id}`.
 *  `lang` is `"prql"` (compiles to the target's dialect) or `"raw"` (target-native text verbatim);
 *  `target` is `"platform"` or `"datasource:<name>"`; `params` are the declared `$var` names bound
 *  at run time. The text is NOT executed at save (a save is not a run). */
export function saveQuery(args: {
  id: string;
  name?: string;
  description?: string;
  lang: QueryLang;
  text: string;
  target: QueryTarget;
  params?: string[];
  ts?: number;
}): Promise<{ id: string }> {
  return mcp<{ id: string }>("query.save", {
    id: args.id,
    name: args.name,
    description: args.description,
    lang: args.lang,
    text: args.text,
    target: args.target,
    params: args.params ?? [],
    ...(args.ts !== undefined ? { ts: args.ts } : {}),
  });
}

/** Soft-delete a saved query (idempotent tombstone). Mirrors `query.delete`. */
export function deleteQuery(id: string, ts?: number): Promise<void> {
  return mcp<void>("query.delete", { id, ...(ts !== undefined ? { ts } : {}) }).then(() => undefined);
}

/** Compile a query's text for the target's dialect WITHOUT running it (`query.compile`). The
 *  authoring dry-run: returns the compiled SQL (or the verbatim raw text) so the editor can show a
 *  live preview. No rows, no cap beyond `mcp:query.compile:call`. */
export function compileQuery(args: {
  lang: QueryLang;
  text: string;
  target: QueryTarget;
}): Promise<QueryCompileResult> {
  return mcp<QueryCompileResult>("query.compile", {
    lang: args.lang,
    text: args.text,
    target: args.target,
  });
}

/** Run a saved (by id) or inline (`lang+text+target`) query (`query.run`). Returns `{columns, rows}`.
 *
 *  NO-WIDENING (rule 5): the caller must ALSO hold the target's underlying cap (`store.query` for
 *  platform, `federation.query` for a datasource). Holding `query.run` alone is denied — the
 *  headline no-widening deny, surfaced as a generic "not permitted". A `vars` map binds `$var`
 *  names through the engine's real param path (missing/extra is a typed author error). */
export function runQuery(args: {
  id?: string;
  lang?: QueryLang;
  text?: string;
  target?: QueryTarget;
  params?: string[];
  vars?: Record<string, unknown>;
  ts?: number;
}): Promise<QueryRunResult> {
  return mcp<QueryRunResult>("query.run", {
    ...(args.id !== undefined ? { id: args.id } : {}),
    ...(args.lang !== undefined ? { lang: args.lang } : {}),
    ...(args.text !== undefined ? { text: args.text } : {}),
    ...(args.target !== undefined ? { target: args.target } : {}),
    ...(args.params !== undefined ? { params: args.params } : {}),
    ...(args.vars !== undefined ? { vars: args.vars } : {}),
    ...(args.ts !== undefined ? { ts: args.ts } : {}),
  });
}
