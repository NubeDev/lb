// The system-map API client — one call per export, mirroring the gateway's `/system/*` routes and the
// host `system_overview`/`system_topology` verbs 1:1 (system-map scope). The UI never calls `invoke`
// directly; it goes through these named verbs (FILE-LAYOUT frontend rules). Both calls are
// **admin-gated** server-side (`mcp:system.overview/topology:call`, granted to the workspace-admin
// role only) and READ-ONLY. The workspace comes from the session token, never an argument (§7).

import type { SystemOverview, SystemTopology } from "./system.types";
import { invoke } from "@/lib/ipc/invoke";

/** The per-subsystem status grid for the session workspace. Mirrors `system.overview`. */
export function systemOverview(): Promise<SystemOverview> {
  return invoke<SystemOverview>("system_overview");
}

/** Nodes + wiring edges for the react-flow graph. Mirrors `system.topology`. */
export function systemTopology(): Promise<SystemTopology> {
  return invoke<SystemTopology>("system_topology");
}
