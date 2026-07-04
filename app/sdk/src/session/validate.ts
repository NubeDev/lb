// Prove a REHYDRATED session is still live before the UI trusts it. `restore()` alone only reads the
// stored token back — it can't know the token still verifies against the node it names. That gap bit
// the browser preview: the preview gateway is in-memory (`make dev` restarts wipe its store AND
// re-key it), so after a restart the stored token is dead, yet the shell silently showed an empty
// channel list ("my channels vanished") instead of falling back to login. See
// `docs/debugging/app/stale-preview-session-shows-empty.md`.
//
// The probe is one cheap authenticated read (`GET /workspaces` — every member holds `workspace.list`,
// and it's what the shell reads on boot anyway). We classify the outcome, not the payload:
//   • 401            → the token no longer verifies (expired / node re-keyed). Dead session — drop it.
//   • network error  → the named node is unreachable. For a THROWAWAY preview node a session we can't
//                      verify is worthless (there's nothing durable behind it), so drop and let the
//                      user re-login (one prefilled click). A device build pointed at a durable node
//                      would rather keep an offline session — that policy is the caller's, via
//                      `onUnreachable` (default: drop).
//   • anything else  → the node answered and the token authenticated (403 = valid token, missing cap;
//                      2xx = fine). The session is LIVE — keep it. A cap deny is not staleness.
//
// This lives beside the session store (not in it): the store owns storage, not the network. The
// client composes the two — see `client/create.ts`.

import { fetchOf, type GatewayConfig } from "../client/config";

/** What a liveness probe decided about the rehydrated token. */
export type SessionLiveness = "live" | "dead" | "unreachable";

/** Probe whether the current token still authenticates against `config.baseUrl`. Pure classification
 *  — it drops nothing; the caller acts on the verdict. `""` token (logged out) is trivially "dead". */
export async function probeSession(config: GatewayConfig): Promise<SessionLiveness> {
  if (!config.getToken()) return "dead";
  try {
    const res = await fetchOf(config)(`${config.baseUrl}/workspaces`, {
      headers: { authorization: `Bearer ${config.getToken()}` },
    });
    // 401 is the only "your token is bad" answer; every other reply means the node authenticated us
    // (403 = valid token without the cap, 2xx = fine) — the session is live either way.
    return res.status === 401 ? "dead" : "live";
  } catch {
    // fetch rejected → the node is down/unreachable, not that the token is bad.
    return "unreachable";
  }
}
