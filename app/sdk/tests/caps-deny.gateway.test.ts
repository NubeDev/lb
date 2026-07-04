// MANDATORY capability-deny tests (testing-scope §2.1): a real signed token WITHOUT the write
// grant gets a typed deny through the app client — surfaced as data (`InvokeError.isDenied`),
// never a crash, and never a silent empty state. One deny per verb this slice ships a write for,
// plus the read-side deny (a token with NO channel caps can't even list).

import { describe, expect, it } from "vitest";

import { InvokeError, type ChannelRecord } from "../src/index";
import { realClient, signInWithCaps } from "./harness";

// Read-only channel caps: list + history pass (`bus:chan/*:sub`), every write is denied.
const READ_ONLY = ["bus:chan/*:sub", "mcp:channel.history:call", "mcp:channel.list:call"];

describe("capability deny through the app seam", () => {
  it("denies_channel_post_without_the_pub_grant", async () => {
    const writer = realClient();
    await writer.login("ana", "app-deny-a");
    await writer.invoke("channel_create", { channel: "general" });

    const reader = realClient();
    await signInWithCaps(reader, "mallory", "app-deny-a", READ_ONLY);

    // Reads work — the token is real and holds the sub grant…
    const rooms = await reader.invoke<ChannelRecord[]>("channel_list");
    expect(rooms.map((c) => c.id)).toContain("general");

    // …but the post is refused with the host's typed capability deny.
    const denied = await reader
      .invoke("channel_post", {
        channel: "general",
        item: { id: "nope", channel: "general", author: "user:mallory", body: "hi", ts: 1 },
      })
      .catch((e: unknown) => e);
    expect(denied).toBeInstanceOf(InvokeError);
    expect((denied as InvokeError).isDenied).toBe(true);
  });

  it("denies_channel_create_without_the_pub_grant", async () => {
    const reader = realClient();
    await signInWithCaps(reader, "mallory", "app-deny-b", READ_ONLY);
    const denied = await reader
      .invoke("channel_create", { channel: "new-room" })
      .catch((e: unknown) => e);
    expect(denied).toBeInstanceOf(InvokeError);
    expect((denied as InvokeError).isDenied).toBe(true);
  });

  it("denies_channel_list_and_history_without_the_sub_grant", async () => {
    const stranger = realClient();
    await signInWithCaps(stranger, "mallory", "app-deny-c", ["mcp:prefs.get:call"]);
    const list = await stranger.invoke("channel_list").catch((e: unknown) => e);
    expect((list as InvokeError).isDenied).toBe(true);
    const history = await stranger
      .invoke("channel_history", { channel: "general" })
      .catch((e: unknown) => e);
    expect((history as InvokeError).isDenied).toBe(true);
  });

  it("denies_mcp_call_without_the_tool_cap", async () => {
    const stranger = realClient();
    await signInWithCaps(stranger, "mallory", "app-deny-d", ["bus:chan/*:sub"]);
    const denied = await stranger
      .invoke("mcp_call", { tool: "channel.post", args: { channel: "general" } })
      .catch((e: unknown) => e);
    expect(denied).toBeInstanceOf(InvokeError);
    // The host refuses the un-granted tool; 403 is the capability deny.
    expect((denied as InvokeError).isDenied).toBe(true);
  });
});
