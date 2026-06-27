// The session api client — the `login` verb (collaboration scope, slice 1). Mirrors the gateway
// `POST /login` one-to-one. The reply carries the signed token + resolved principal/workspace, which
// the caller hands to `setSession`. Logout is local (drop the token) — there is no server session to
// destroy, the token simply expires.

import type { Session } from "./session.types";
import { invoke } from "@/lib/ipc/invoke";

/** Log in as `user` into `workspace`. Returns the verified session (token + principal + workspace).
 *  Mirrors the gateway `login` route; the dev credential check is server-side (no password yet). */
export function login(user: string, workspace: string): Promise<Session> {
  return invoke<Session>("login", { user, workspace });
}
