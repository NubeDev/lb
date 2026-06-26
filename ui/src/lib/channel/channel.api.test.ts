// The channel api client over the in-memory node fake: it mirrors the real node's contract
// (ordered history, idempotent on id, workspace-scoped). When the same client runs in the
// Tauri shell it hits the real node commands of the same name — so these guarantees are the
// node's, asserted here against the stand-in.

import { describe, expect, it } from "vitest";

import { history, post } from "./channel.api";
import type { Item } from "./channel.types";

function item(id: string, body: string, ts: number): Item {
  return { id, channel: "general", author: "u", body, ts };
}

describe("channel.api over the fake node", () => {
  it("history returns posts oldest→newest", async () => {
    await post("acme", "general", item("b", "second", 2));
    await post("acme", "general", item("a", "first", 1));
    const got = await history("acme", "general");
    expect(got.map((m) => m.body)).toEqual(["first", "second"]);
  });

  it("re-posting the same id is idempotent", async () => {
    await post("acme", "general", item("x", "once", 1));
    await post("acme", "general", item("x", "once", 1));
    expect(await history("acme", "general")).toHaveLength(1);
  });

  it("history is workspace-scoped — B never sees A's posts", async () => {
    await post("acme", "general", item("a", "secret of acme", 1));
    expect(await history("other", "general")).toEqual([]);
  });
});
