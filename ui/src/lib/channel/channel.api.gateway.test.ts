// The channel api client over the REAL spawned gateway (no fake — CLAUDE §9). The client mirrors the
// real node's contract (ordered history, idempotent on id, workspace-scoped); here we assert those
// guarantees end-to-end against `POST|GET /channels/{cid}/messages`. Workspace scoping is real: each
// case signs into its own workspace and the node derives the workspace from the token (the hard wall
// §7), so the `ws` argument the client passes is dropped server-side.

import { describe, expect, it, beforeAll } from "vitest";

import { history, post } from "./channel.api";
import type { Item } from "./channel.types";
import { useRealGateway, signInReal } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `chanapi-${n++}`;

function item(id: string, body: string, ts: number): Item {
  return { id, channel: "general", author: "u", body, ts };
}

beforeAll(() => useRealGateway());

describe("channel.api over the real gateway", () => {
  it("history returns posts oldest→newest", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await post(ws, "general", item("b", "second", 2));
    await post(ws, "general", item("a", "first", 1));
    const got = await history(ws, "general");
    expect(got.map((m) => m.body)).toEqual(["first", "second"]);
  });

  it("re-posting the same id is idempotent", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await post(ws, "general", item("x", "once", 1));
    await post(ws, "general", item("x", "once", 1));
    expect(await history(ws, "general")).toHaveLength(1);
  });

  it("history is workspace-scoped — B never sees A's posts", async () => {
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    await post(wsA, "general", item("a", "secret of acme", 1));

    const wsB = nextWs();
    await signInReal("user:bob", wsB);
    expect(await history(wsB, "general")).toEqual([]);
  });
});
