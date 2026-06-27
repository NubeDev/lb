// Shared helper for the real-gateway tests: point `invoke` at the spawned server and log in for a
// real signed session. Importing this from a `*.gateway.test.tsx` gives a `signInReal(workspace)`
// that mints a real token (carrying the dev claim set — which includes the data-console caps) and
// stores it, so every subsequent `invoke` call hits the real backend with a real bearer token.

import { inject, vi } from "vitest";

import { login } from "@/lib/session/session.api";
import { setSession } from "@/lib/session/session.store";

/** Make `invoke` take the real HTTP path to the spawned gateway (stub `VITE_GATEWAY_URL` to its URL).
 *  Call once per test file (idempotent). */
export function useRealGateway(): void {
  vi.stubEnv("VITE_GATEWAY_URL", inject("gatewayUrl"));
}

/** Log in `user` into `workspace` against the real gateway and store the session. Returns the session
 *  (token + principal + caps). A unique workspace per test keeps the shared real backend isolated. */
export async function signInReal(user: string, workspace: string) {
  const session = await login(user, workspace);
  setSession(session);
  return session;
}
