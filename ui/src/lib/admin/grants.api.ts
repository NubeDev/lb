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

// ── access-console scope — resolved effective caps WITH provenance + the live-token revoke lever. ──

/** Where a resolved cap came from — mirrors `lb_authz::CapSource` (serde `kind`-tagged). */
export type CapSource =
  | { kind: "direct" }
  | { kind: "role"; name: string }
  | { kind: "team"; name: string };

/** One resolved cap plus the distinct edges that contributed it. Mirrors `lb_authz::SourcedCap`. */
export interface SourcedCap {
  cap: string;
  source: CapSource[];
}

/**
 * Resolve `subject`'s effective caps WITH provenance (direct / role:r / via team:t). Mirrors
 * `authz.resolve` — the sourced twin of the session-mint fold, so the displayed set and the enforced
 * set cannot drift. Admin-only.
 */
export function resolveCaps(subject: string): Promise<SourcedCap[]> {
  return invoke<SourcedCap[]>("authz_resolve", { subject });
}

/**
 * Kill `subject`'s live tokens (the verify-path marker) AND tombstone its grants — a full immediate
 * lockout. Mirrors `authz.revoke-tokens`. The single-node case is instant; the multi-node worst case
 * is bounded by the token TTL. Returns the number of grants tombstoned.
 */
export function revokeTokens(subject: string): Promise<{ grants_revoked: number }> {
  return invoke<{ grants_revoked: number }>("authz_revoke_tokens", { subject });
}
