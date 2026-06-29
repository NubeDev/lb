// The store-schema reader — the `store.schema` verb (the workspace's tables + columns). Extracted from
// `lib/dashboard/sql.api.ts` so it is a SHARED, feature-neutral reader: BOTH the dashboard SQL builder
// (Builder dropdowns) AND the rules data explorer (click-to-insert tree) consume it. Schema is a generic
// store concern, not a dashboard one — hence its own named module (rules-editor-ux scope).
//
// Rides the host-mediated `POST /mcp/call` bridge (the `mcp_call` invoke verb), capability-gated
// server-side (`mcp:store.schema:call`); the workspace comes from the session token (the hard wall, §7),
// never an argument. One responsibility per file (FILE-LAYOUT): this is only the schema read + its types.

import { invoke } from "@/lib/ipc/invoke";

/** One column of a table as `store.schema` reports it. */
export interface SchemaColumn {
  name: string;
  type: string;
}

/** One table in the workspace + its columns. */
export interface SchemaTable {
  name: string;
  columns: SchemaColumn[];
}

/** The workspace schema (every table + its columns) — the shared dropdown/explorer source. */
export interface Schema {
  tables: SchemaTable[];
}

/** Read the workspace's schema (tables + columns). Mirrors `store.schema`. */
export function readSchema(): Promise<Schema> {
  return invoke<Schema>("mcp_call", { tool: "store.schema", args: {} });
}
