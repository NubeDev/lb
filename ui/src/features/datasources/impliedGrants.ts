// The implied grants a datasource registration carries (rules-workbench scope, Phase 3) — a pure,
// named concept: given the Add form fields, what `net:*` + `secret:*` grants does registering this
// source imply? These are DISPLAY ONLY — the real approval is the host's install-grant record the
// federation extension is enforced against pre-connect. The form shows "this is what you're approving".
//
// One responsibility (form fields → grant strings), one file (FILE-LAYOUT) — NOT a utils bucket.

import type { AddDatasource } from "@/lib/datasources";

/** Parse `host:port` out of an endpoint like `tsdb.acme:5432`. Returns `null` if it has no `:port`. */
function hostPort(endpoint: string): { host: string; port: string } | null {
  const i = endpoint.lastIndexOf(":");
  if (i <= 0 || i === endpoint.length - 1) return null;
  return { host: endpoint.slice(0, i), port: endpoint.slice(i + 1) };
}

/** The grant strings registering `{name, endpoint}` implies:
 *   - `net:tls:{host}:{port}:connect` — the pre-connect network reach the endpoint needs;
 *   - `secret:federation/{name}:get` — reading the DSN ref the host stores.
 *  An endpoint without a parseable `host:port` yields only the secret grant (the net one is shown once
 *  the endpoint is well-formed). */
export function impliedGrants(form: Pick<AddDatasource, "name" | "endpoint">): string[] {
  const grants: string[] = [];
  const hp = hostPort(form.endpoint.trim());
  if (hp) grants.push(`net:tls:${hp.host}:${hp.port}:connect`);
  if (form.name.trim()) grants.push(`secret:federation/${form.name.trim()}:get`);
  return grants;
}
