// Regression for the stale-preview-session trap (see `docs/debugging/app/
// stale-preview-session-shows-empty.md`): the browser preview persists its session in localStorage,
// but the preview gateway is an IN-MEMORY `test_gateway` — every `make dev` restart wipes its store
// AND mints a fresh signing key. So after a restart the persisted token is dead, yet the old boot
// path (`session.restore()`) rehydrated it blindly and the shell rendered an empty channel list
// ("my channels vanished") instead of a login prompt.
//
// The fix is `client.restore()`: rehydrate, then PROVE the token still verifies (`probeSession` →
// `GET /workspaces`) and drop a session the node rejects (401) or, with `onUnreachable: "drop"`, one
// whose node is unreachable. This pins that contract against the REAL spawned gateway (rule 9 — real
// tokens, real verify path, real network error; memory storage is a storage adapter, not a fake).
//
// How each case is produced WITHOUT faking the backend:
//   • live        → a REAL login against the running gateway; its own token round-trips.
//   • dead (401)  → a syntactically-shaped but unsigned token hits the running gateway's REAL verify
//                   path and is genuinely rejected 401 (an invalid credential, not a fake server).
//   • unreachable → the same real client pointed at a closed port — a genuine fetch failure.

import { describe, expect, it } from "vitest";
import { inject } from "vitest";

import {
  createGatewayClient,
  memorySessionStorage,
  type GatewayClientOptions,
  type SessionStorage,
  type StoredSessions,
} from "../src/index";

/** A memory storage adapter PRE-SEEDED with one persisted session — exactly what the keychain/
 *  localStorage would hand back on boot. Real token or a dead one, per the caller. */
function storageHolding(sessions: StoredSessions): SessionStorage {
  const inner = memorySessionStorage();
  void inner.save(sessions);
  return inner;
}

/** A stored session for `ws` carrying `token` — the shape `restore()` rehydrates. */
function stored(ws: string, token: string): StoredSessions {
  return {
    active: ws,
    sessions: { [ws]: { token, principal: "user:ada", workspace: ws, caps: [] } },
  };
}

function clientWith(storage: SessionStorage, extra?: Partial<GatewayClientOptions>) {
  return createGatewayClient({ baseUrl: inject("gatewayUrl"), storage, ...extra });
}

describe("client.restore() drops a rehydrated-but-invalid session (stale-preview regression)", () => {
  it("keeps a LIVE session: a real token the running gateway still honours survives restore", async () => {
    // Log in for real to mint a genuine token, capture what the shell would have persisted.
    const source = clientWith(memorySessionStorage());
    const issued = await source.login("ada", "restore-live-ws");
    expect(issued.token).not.toBe("");

    // A fresh client boots against the SAME running gateway with that session pre-persisted.
    const rebooted = clientWith(storageHolding(stored("restore-live-ws", issued.token)));
    const survived = await rebooted.restore();

    // The node honoured the token → session is live and kept.
    expect(survived?.workspace).toBe("restore-live-ws");
    expect(rebooted.session.current()?.token).toBe(issued.token);
  });

  it("drops a DEAD session: a token the running gateway rejects (401) falls to logged-out", async () => {
    // A shaped-but-unsigned token — the REAL gateway verify path rejects it 401 (invalid credential,
    // not a fake backend). This is the "node re-keyed / forgot the token after restart" case.
    const deadToken = "not-a-real-signed-token.aaaa.bbbb";
    const client = clientWith(storageHolding(stored("restore-dead-ws", deadToken)));

    let notifications = 0;
    client.session.subscribe(() => (notifications += 1));

    const survived = await client.restore();

    // Rejected → session dropped, and subscribers (→ `useSession` → login screen) were notified.
    expect(survived).toBeNull();
    expect(client.session.current()).toBeNull();
    expect(notifications).toBeGreaterThan(0);
  });

  it("drops an UNREACHABLE session when onUnreachable=drop (throwaway preview node is down)", async () => {
    // Point at a closed port on localhost — a real network failure, no server to answer. This is the
    // exact `make dev` restart window: the token can't be verified, so for a throwaway node we drop.
    const dead = createGatewayClient({
      baseUrl: "http://127.0.0.1:1", // port 1 — nothing listens; fetch rejects
      storage: storageHolding(stored("restore-down-ws", "any-token")),
      onUnreachable: "drop",
    });

    const survived = await dead.restore();
    expect(survived).toBeNull();
    expect(dead.session.current()).toBeNull();
  });

  it("KEEPS an unreachable session when onUnreachable=keep (durable-node offline policy)", async () => {
    // The device-build policy: a durable node that's merely offline should not log you out.
    const kept = createGatewayClient({
      baseUrl: "http://127.0.0.1:1",
      storage: storageHolding(stored("restore-offline-ws", "any-token")),
      onUnreachable: "keep",
    });

    const survived = await kept.restore();
    expect(survived?.workspace).toBe("restore-offline-ws");
    expect(kept.session.current()?.workspace).toBe("restore-offline-ws");
  });
});
