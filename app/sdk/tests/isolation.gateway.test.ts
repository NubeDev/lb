// MANDATORY workspace-isolation test (testing-scope §2.2): two workspaces, two real tokens; one's
// channels are invisible — and unreachable — to the other through the app client seam.

import { describe, expect, it } from "vitest";

import { InvokeError, type ChannelRecord, type Item } from "../src/index";
import { realClient } from "./harness";

describe("workspace isolation through the app seam", () => {
  it("workspace_b_cannot_see_or_read_workspace_a_channels", async () => {
    const a = realClient();
    const b = realClient();
    await a.login("ana", "app-iso-a");
    await b.login("bob", "app-iso-b");

    await a.invoke("channel_create", { channel: "secret" });
    await a.invoke("channel_post", {
      channel: "secret",
      item: { id: "m1", channel: "secret", author: "user:ana", body: "ours only", ts: 1 },
    });

    // Invisible in B's list…
    const bList = await b.invoke<ChannelRecord[]>("channel_list");
    expect(bList.map((c) => c.id)).not.toContain("secret");

    // …and B reading "secret" by name lands in B's OWN namespace (workspace-first key scoping):
    // never A's rows. Either an empty history or a deny is acceptable — A's data is not.
    const bRead = await b
      .invoke<Item[]>("channel_history", { channel: "secret" })
      .catch((e: unknown) => e);
    if (bRead instanceof InvokeError) {
      expect(bRead.isDenied).toBe(true);
    } else {
      expect(bRead as Item[]).toEqual([]);
    }

    // A still reads its own row — the wall blocks B, not the owner.
    const aRead = await a.invoke<Item[]>("channel_history", { channel: "secret" });
    expect((aRead as Item[]).map((i) => i.id)).toContain("m1");
  });
});
