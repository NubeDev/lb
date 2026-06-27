// The DB-browser API client — one call per export, mirroring the gateway's `store.*` routes and the
// host `store_*_view` verbs 1:1 (data-console scope). The UI never calls `invoke` directly; it goes
// through these named verbs (FILE-LAYOUT frontend rules). Every call is **admin-gated** server-side
// (the `mcp:store.*:call` caps, granted to the workspace-admin role only) — these verbs relax the
// per-record membership gate, so they are admin-only and READ-ONLY. The workspace comes from the
// session token, never an argument (the hard wall, §7).

import type { Graph, Page, TableCount } from "./data.types";
import { invoke } from "@/lib/ipc/invoke";

/** List the workspace's tables + row counts (the picker). Mirrors `store.tables`. */
export function listTables(): Promise<TableCount[]> {
  return invoke<TableCount[]>("store_tables");
}

/** Scan a bounded page of `table`'s raw rows, starting after `cursor` (the grid). The server hard-
 *  caps `limit`; the page carries the next cursor (or `null` at the end). Mirrors `store.scan`. */
export function scanTable(table: string, limit?: number, cursor?: string): Promise<Page> {
  return invoke<Page>("store_scan", { table, limit, cursor });
}

/** Read a depth/fan-out-bounded graph slice seeded from a `table` and/or a record `id` (react-flow).
 *  Click-to-expand a node by calling again with its `id`. Mirrors `store.graph`. */
export function readGraph(table?: string, id?: string, depth?: number): Promise<Graph> {
  return invoke<Graph>("store_graph", { table, id, depth });
}
