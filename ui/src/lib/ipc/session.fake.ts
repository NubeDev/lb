// The in-memory session fake — `login` mints a deterministic fake token (TEST-ONLY; the real path
// hits the gateway's `login` route). Contract-identical to the gateway `LoginReply`: a token plus the
// resolved principal + workspace. The token is opaque to the fake surfaces; they read the *workspace*
// from the session store (which the test sets from this reply), exactly as the real gateway derives
// it from the verified token. Returns `null` for any command it does not own (fake-chain convention).

import type { Session } from "@/lib/session/session.types";
import { ADMIN_CAPS } from "@/lib/session/admin-caps";

export function sessionFakeInvoke<T>(cmd: string, args?: Record<string, unknown>): T | null {
  if (cmd !== "login") return null;
  const { user, workspace } = args as { user: string; workspace: string };
  // The dev-login is a workspace admin — mirrors the gateway's `dev_claims` (admin-console scope).
  // The fake hands back the same admin cap grant so the UI's cap-gated display matches the real
  // session. Tests that need a non-admin override `caps` directly via `setSession`.
  const reply: Session = {
    token: `fake-token:${user}:${workspace}`,
    principal: user,
    workspace,
    caps: ADMIN_CAPS,
  };
  return reply as T;
}
