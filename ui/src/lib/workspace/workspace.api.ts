// The workspace api client — one call per export, mirroring `lb_host::workspace_list` /
// `workspace_create` and the gateway `GET|POST /workspaces` one-to-one (collaboration scope, slice 2).

import type { WorkspaceRecord } from "./workspace.types";
import { invoke } from "@/lib/ipc/invoke";

/** List the workspaces in the node directory (for the switcher). Mirrors `workspace_list`. */
export function listWorkspaces(): Promise<WorkspaceRecord[]> {
  return invoke<WorkspaceRecord[]>("workspace_list", {});
}

/** Register a workspace (id + display name) in the directory. Mirrors `workspace_create`. */
export function createWorkspace(ws: string, name: string): Promise<WorkspaceRecord> {
  return invoke<WorkspaceRecord>("workspace_create", { ws, name });
}
