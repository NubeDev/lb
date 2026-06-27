// The grants admin api client — read + assign/revoke a subject's capabilities. One call per export,
// mirroring the host `grants.*` verbs and the gateway `/admin/grants` routes 1:1. A subject is
// `kind:name` (`user:bob` / `team:eng`); assigning a role is a grant of the synthetic cap
// `role:<name>` (see roles.api for defining roles).

import { invoke } from "@/lib/ipc/invoke";

/** List the caps granted to `subject` (`user:…` / `team:…`). Mirrors `grants.list`. */
export function listGrants(subject: string): Promise<string[]> {
  return invoke<string[]>("grants_list", { subject });
}

/** Grant `cap` to `subject`. Mirrors `grants.assign` (no-widening enforced server-side). */
export function assignGrant(subject: string, cap: string): Promise<void> {
  return invoke<void>("grants_assign", { subject, cap });
}

/** Revoke `cap` from `subject` (idempotent tombstone). Mirrors `grants.revoke`. */
export function revokeGrant(subject: string, cap: string): Promise<void> {
  return invoke<void>("grants_revoke", { subject, cap });
}
