// The datasource API client — one call per export, mirroring the gateway's `datasource.*` routes and
// the shipped host verbs 1:1 (rules-workbench scope, Phase 3). The UI never calls `invoke` directly; it
// goes through these named verbs (FILE-LAYOUT frontend rules). Each is capability-gated server-side; the
// workspace + principal come from the session token (the hard wall, §7), never an argument. The DSN is
// supplied ONLY on `addDatasource` and never read back — no response carries it (§6.7 / redaction rule).

import type {
  AddDatasource,
  DatasourceSummary,
  FederationQueryResult,
  ProbeResult,
} from "./datasource.types";
import { invoke } from "@/lib/ipc/invoke";

/** The list-row wire shape the gateway returns (snake_case `secret_ref`, NEVER a `dsn`). */
interface RawSummary {
  name: string;
  kind: string;
  endpoint: string;
  secret_ref: string;
}

/** The registered sources in the workspace — kind + endpoint + a redacted secret ref, never a DSN.
 *  Mirrors `datasource.list`. */
export function listDatasources(): Promise<DatasourceSummary[]> {
  return invoke<{ datasources: RawSummary[] }>("datasource_list", {}).then((r) =>
    r.datasources.map((d) => ({
      name: d.name,
      kind: d.kind,
      endpoint: d.endpoint,
      secretRef: d.secret_ref,
    })),
  );
}

/** Register a source. The DSN flows page → host → secret store and is never returned. Mirrors
 *  `datasource.add`. */
export function addDatasource(input: AddDatasource): Promise<void> {
  return invoke<void>("datasource_add", { ...input });
}

/** Drop a source record. Mirrors `datasource.remove`. */
export function removeDatasource(name: string): Promise<void> {
  return invoke<void>("datasource_remove", { name });
}

/** A real connectivity probe via the supervised federation sidecar. Green on `{ok:true}`; a thrown
 *  error (the gateway's non-200 — a sidecar fault / refused endpoint / missing source) is an HONEST RED
 *  probe, surfaced verbatim, never a fabricated green. Mirrors `datasource.test`. */
export async function testDatasource(name: string): Promise<ProbeResult> {
  try {
    const r = await invoke<{ ok: boolean }>("datasource_test", { name });
    return { ok: r.ok === true };
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e) };
  }
}

/** Run a read-only SELECT against the registered external `source` via the host-mediated `mcp/call`
 *  bridge → `federation.query`. SELECT-only is enforced host-side AND in the sidecar; the workspace
 *  + principal come from the session token (the wall, §7). Returns the sidecar's `{columns, rows}`. */
export function runFederationQuery(
  source: string,
  sql: string,
): Promise<FederationQueryResult> {
  return invoke<FederationQueryResult>("mcp_call", {
    tool: "federation.query",
    args: { source, sql },
  });
}
