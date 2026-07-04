// Regression for the shell's "stuck on the login screen" bug (session doc §Debugging): the shell
// held its GatewayClient in a module singleton (`app/shell/src/lib/client.ts`) but was REBUILDING
// it whenever the node URL changed. `useSession` subscribes to `client.session` (the store), so a
// rebuilt client swapped the store out from under the live subscription — login succeeded on the
// NEW client's store while the mounted screen still listened to the OLD one, and the UI never left
// the login screen.
//
// The shell has no component runner yet (RN jest/babel harness is deferred to app-extensions), so
// this pins the invariant one layer down, against the REAL gateway: for ONE client instance, a
// subscriber attached to `client.session` BEFORE login fires when login completes and observes the
// active session — and a second subscriber on the SAME client sees it too. That is exactly the
// contract `useSession` relies on and the rebuild broke. It also asserts the store identity is
// stable across a login (the object `useSession` closed over is never replaced).
//
// rule 9: real spawned gateway, real signed token, no fakes (memory storage is a storage adapter,
// not a fake backend — see harness.ts / testing §0).

import { describe, expect, it } from "vitest";

import { realClient } from "./harness";

describe("client session store stays observable across login (singleton regression)", () => {
  it("fires a pre-login subscriber and reflects the session on the same client", async () => {
    const client = realClient();

    // The store `useSession` would subscribe to, captured ONCE — as the shell captures it when the
    // client is built. If the client were rebuilt on login, this reference would go stale.
    const store = client.session;
    expect(store.current()).toBeNull(); // login screen state

    let notifications = 0;
    const seenWorkspaces: (string | null)[] = [];
    const unsubscribe = store.subscribe(() => {
      notifications += 1;
      seenWorkspaces.push(store.current()?.workspace ?? null);
    });

    // A SECOND subscriber on the same client — the switcher/nav also listen. Both must wake.
    let secondNotified = false;
    const unsub2 = client.session.subscribe(() => {
      secondNotified = true;
    });

    const reply = await client.login("ada", "app-singleton-ws");
    expect(reply.token).not.toBe("");

    // The subscription attached before login fired, and the store it was attached to now holds the
    // real session — no orphaned listener, no swapped-out store.
    expect(notifications).toBeGreaterThan(0);
    expect(seenWorkspaces).toContain("app-singleton-ws");
    expect(secondNotified).toBe(true);

    // Same client instance, same store object: what the UI closed over is what login updated.
    expect(client.session).toBe(store);
    expect(store.current()?.workspace).toBe("app-singleton-ws");
    expect(store.token()).toBe(reply.token);

    unsubscribe();
    unsub2();

    // After unsubscribe, a further mutation on the same client does not notify the dropped listener
    // (proves subscribe returns a working unsubscribe — the shell relies on it in useEffect cleanup).
    const before = notifications;
    client.session.logout();
    expect(notifications).toBe(before);
    expect(client.session.current()).toBeNull();
  });
});
