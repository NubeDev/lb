// The system-map API client — one call per export, mirroring the gateway's `/system/*` routes and the
// host `system_overview`/`system_topology` verbs 1:1 (system-map scope). The UI never calls `invoke`
// directly; it goes through these named verbs (FILE-LAYOUT frontend rules). Both calls are
// **admin-gated** server-side (`mcp:system.overview/topology:call`, granted to the workspace-admin
// role only) and READ-ONLY. The workspace comes from the session token, never an argument (§7).

import type {
  AcpInfo,
  SubsystemDetail,
  SystemOverview,
  SystemTools,
  SystemTopology,
} from "./system.types";
import { invoke } from "@/lib/ipc/invoke";

/** The per-subsystem status grid for the session workspace. Mirrors `system.overview`. */
export function systemOverview(): Promise<SystemOverview> {
  return invoke<SystemOverview>("system_overview");
}

/** Nodes + wiring edges for the react-flow graph. Mirrors `system.topology`. */
export function systemTopology(): Promise<SystemTopology> {
  return invoke<SystemTopology>("system_topology");
}

/** The full detail of one subsystem `id` (its card + a subsystem-specific `extra` blob). The detail
 *  view a no-page card drills into. Mirrors `system.subsystem`. */
export function systemSubsystem(id: string): Promise<SubsystemDetail> {
  return invoke<SubsystemDetail>("system_subsystem", { id });
}

/** The full catalog of MCP tools reachable for the session workspace (host-native + extension), with
 *  descriptions — the read behind the MCP service page. Mirrors `system.tools`. Admin-gated. */
export function systemTools(): Promise<SystemTools> {
  return invoke<SystemTools>("system_tools");
}

/** The ACP adapter's static protocol/capability facts — the read behind the ACP service page. Mirrors
 *  `system.acp`. Admin-gated. */
export function systemAcp(): Promise<AcpInfo> {
  return invoke<AcpInfo>("system_acp");
}
