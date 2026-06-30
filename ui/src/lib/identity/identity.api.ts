// The identity api client — one call per export, mirroring `lb_host::identity_*` and the gateway
// `/admin/identities*` routes 1:1 (global-identity scope). The global identity directory lives in a
// reserved system namespace; these verbs are gated `mcp:identity.manage:call`.

import { invoke } from "@/lib/ipc/invoke";

export interface IdentityView {
  sub: string;
  display_name?: string;
  created_ts: number;
}

export interface IdentityWorkspace {
  ws: string;
  name: string;
}

/** Every global identity. Mirrors `identity.list`. */
export function listIdentities(): Promise<IdentityView[]> {
  return invoke<IdentityView[]>("identity_list", {});
}

/** Provision a global identity (in NO workspace). Mirrors `identity.create`. */
export function createIdentity(sub: string, displayName?: string): Promise<IdentityView> {
  return invoke<IdentityView>("identity_create", { sub, display_name: displayName });
}

/** Read one identity. Mirrors `identity.get`. */
export function getIdentity(sub: string): Promise<IdentityView | null> {
  return invoke<IdentityView | null>("identity_get", { sub });
}

/** The workspaces this identity belongs to (drives login + the switcher). Mirrors `identity.workspaces`. */
export function identityWorkspaces(sub: string): Promise<IdentityWorkspace[]> {
  return invoke<IdentityWorkspace[]>("identity_workspaces", { sub });
}
