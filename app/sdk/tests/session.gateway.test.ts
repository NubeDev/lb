// Session flow against the REAL gateway: login → workspace list → switch (token re-mint) →
// the old token stays confined to its own workspace and a cleared/garbage token is rejected.
// (app-shell scope §Testing "Session"; rule 9 — real node, real signed tokens.)

import { describe, expect, it } from "vitest";

import { InvokeError, type ChannelRecord } from "../src/index";
import { realClient } from "./harness";

describe("session over the real gateway", () => {
  it("logs_in_lists_workspaces_and_switches_with_a_reminted_token", async () => {
    const client = realClient();
    const first = await client.login("ana", "app-sess-a");
    expect(first.token).not.toBe("");
    expect(first.workspace).toBe("app-sess-a");
    expect(client.session.current()?.workspace).toBe("app-sess-a");

    // Login registers the workspace in the directory — the switcher's list shows it.
    const listed = await client.invoke<{ ws: string }[]>("workspace_list");
    expect(listed.map((w) => w.ws)).toContain("app-sess-a");

    // Switch = re-mint (re-login) into the second workspace: a DIFFERENT token, new hard wall.
    const second = await client.switchWorkspace("app-sess-b");
    expect(second.workspace).toBe("app-sess-b");
    expect(second.token).not.toBe(first.token);
    expect(client.session.current()?.workspace).toBe("app-sess-b");
    // Both sessions are stored (token per workspace); switching back reuses the stored token.
    expect(client.session.workspaces().sort()).toEqual(["app-sess-a", "app-sess-b"]);
    const back = await client.switchWorkspace("app-sess-a");
    expect(back.token).toBe(first.token);
  });

  it("old_token_stays_confined_to_its_workspace_after_switch", async () => {
    const client = realClient();
    await client.login("ana", "app-old-a");
    await client.invoke("channel_create", { channel: "a-room" });
    await client.switchWorkspace("app-old-b");

    // The active (new) token sees NONE of workspace A's channels — tokens are the wall.
    const inB = await client.invoke<ChannelRecord[]>("channel_list");
    expect(inB.map((c) => c.id)).not.toContain("a-room");

    // Switching back, the old token still only reaches workspace A.
    await client.switchWorkspace("app-old-a");
    const inA = await client.invoke<ChannelRecord[]>("channel_list");
    expect(inA.map((c) => c.id)).toContain("a-room");
  });

  it("rejects_calls_once_the_session_is_dropped_or_the_token_is_garbage", async () => {
    const client = realClient();
    await client.login("ana", "app-drop-a");
    client.session.logout();
    // No token → the gateway refuses; the client surfaces a typed 401, never a crash.
    const noToken = await client.invoke("channel_list").catch((e: unknown) => e);
    expect(noToken).toBeInstanceOf(InvokeError);
    expect((noToken as InvokeError).isUnauthenticated).toBe(true);

    // A token that no longer verifies (tampered) → 401 AND the client drops that session.
    const s = await client.login("ana", "app-drop-a");
    client.session.activate({ ...s, token: s.token.slice(0, -2) + "xx" });
    const bad = await client.invoke("channel_list").catch((e: unknown) => e);
    expect((bad as InvokeError).isUnauthenticated).toBe(true);
    expect(client.session.current()).toBeNull();
  });
});
