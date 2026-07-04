// The rules `channel` messaging handle, driven END-TO-END against a REAL spawned gateway
// (rules-messaging-scope, slice 3; CLAUDE §9 / testing §0 — no fake backend). A rule body runs through
// the real `rules.run` MCP verb, and the channel it wrote is read back through the real `channel.history`
// verb — the same MCP contract the UI/agent use (rule 7). We prove, on the real store/bus/caps path:
//   - a rule's `channel.post` lands a durable message, authored as the caller (never request-supplied);
//   - the worker-kind fence: a rule posting `kind:"agent"` is rejected and spawns NO run (empty channel);
//   - the write is caller-gated: a caller lacking the channel `Pub` cap is denied, opaquely, no write.
// Every datum is a real read/write over the live gateway — nothing is mocked.

import { describe, expect, it, beforeAll } from "vitest";

import { runRule } from "@/lib/rules";
import { history } from "@/lib/channel/channel.api";
import { EXAMPLES } from "./examples/examples";
import { useRealGateway, signInReal, signInWithCaps } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `rules-msg-${n++}`;

beforeAll(() => {
  useRealGateway();
});

describe("rules channel handle (real gateway)", () => {
  it("a rule's channel.post lands a durable message authored as the caller", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);

    await runRule({
      body: `channel.post("ops", #{ id: "from-rule", body: "posted by a rule" });`,
    });

    // Read it back over the real channel.history verb — the post committed to the real store.
    const items = await history(ws, "ops");
    expect(items).toHaveLength(1);
    expect(items[0].body).toBe("posted by a rule");
    expect(items[0].id).toBe("from-rule");
    // Author is FORCED to the signed-in caller (a request-supplied author is ignored).
    expect(items[0].author).toBe("user:ada");
  });

  it("the worker-kind fence rejects a kind:agent post and spawns no run", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);

    // A rule cannot spawn a run — the handle rejects agent/query kinds (Resolved decisions).
    await expect(
      runRule({
        body: `channel.post("ops", #{ kind: "agent", goal: "summarize the logs" });`,
      }),
    ).rejects.toThrow();

    // The fence fired before any write — the channel is empty (no message, no spawned agent run).
    const items = await history(ws, "ops");
    expect(items).toHaveLength(0);
  });

  it("channel.post is caller-gated: no Pub cap ⇒ denied, opaquely, with no write", async () => {
    const ws = nextWs();
    // Sign in WITHOUT `bus:chan/*:pub` (but with the MCP door + `sub` so the read-back still works).
    await signInWithCaps("user:ada", ws, [
      "mcp:rules.run:call",
      "mcp:channel.post:call",
      "mcp:channel.history:call",
      "bus:chan/*:sub",
    ]);

    await expect(
      runRule({ body: `channel.post("ops", #{ body: "should not land" });` }),
    ).rejects.toThrow();

    // The denied post left no partial write — a Sub-capable read shows an empty channel.
    const items = await history(ws, "ops");
    expect(items).toHaveLength(0);
  });

  // The examples catalog promises its bodies actually run (an example that lies is worse than none).
  // Prove EVERY messaging example runs green through the real `rules.run` — the same contract the
  // Examples tab loads into the editor — on a FRESH workspace with NO seeding (they read/write only the
  // inbox/outbox/channel planes, which resolve empty-but-valid on a brand-new workspace).
  it("every messaging example in the catalog runs green with no seeding", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);

    const MESSAGING_EXAMPLES = [
      "inbox-record",
      "inbox-list",
      "inbox-record-then-list",
      "inbox-resolve",
      "outbox-enqueue",
      "outbox-status",
      "channel-post",
      "channel-read",
      "channel-list",
      "escalate-and-notify",
    ];
    for (const id of MESSAGING_EXAMPLES) {
      const ex = EXAMPLES.find((e) => e.id === id);
      expect(ex, `example ${id} exists`).toBeTruthy();
      // Runs without throwing — a valid body against the real store/bus/caps path, zero seeding.
      await runRule({ body: ex!.body });
    }

    // The posting examples landed real messages on `ops` (channel-post + escalate).
    const items = await history(ws, "ops");
    expect(items.some((i) => i.body === "posted from a rule")).toBe(true);
  });
});
