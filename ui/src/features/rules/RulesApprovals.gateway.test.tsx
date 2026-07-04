// The rules approval loop, driven END-TO-END against a REAL spawned gateway (rules-approvals-scope;
// CLAUDE §9 / testing §0 — no fake backend). A rule body runs through the real `rules.run` MCP verb;
// the `needs:approval` item and the HELD gated effect it stages are read back through the real
// `inbox_list` / `outbox_status` verbs; an approval is driven through the real `inbox_resolve` verb;
// and the real approval-release reactor (spawned in the test gateway, 1s tick) is observed flipping
// the held effect to `pending`. Every datum is a real read/write over the live gateway — nothing is
// mocked. We prove, on the real store/bus/caps/reactor path:
//   - `inbox.request_approval` raises a needs:approval item AND stages the effect HELD (not delivered);
//   - approving the item releases the held effect to `pending` (the reactor closes the loop);
//   - the request is caller-gated: a caller lacking the outbox stage cap is denied, opaquely, no write.

import { describe, expect, it, beforeAll } from "vitest";

import { runRule } from "@/lib/rules";
import { listInbox, resolveInbox } from "@/lib/inbox/inbox.api";
import { outboxStatus } from "@/lib/outbox/outbox.api";
import { EXAMPLES } from "./examples/examples";
import { useRealGateway, signInWithCaps } from "@/test/gateway-session";

// The reactor scans this fixed workspace (see test_gateway.rs); each case uses a distinct item id.
const WS = "rules-approvals";

// The full grant a propose-and-approve rule + reviewer needs. (dev-login omits *.set_default etc.;
// we name exactly what this loop touches so the deny case can drop one cap cleanly.)
const CAPS = [
  "mcp:rules.run:call",
  "mcp:inbox.record:call",
  "mcp:inbox.list:call",
  "mcp:inbox.resolve:call",
  "mcp:outbox.enqueue:call",
  "mcp:outbox.status:call",
];

/** Poll `outbox_status` until `predicate` holds (the reactor ticks ~1s), or fail after `tries`. */
async function waitForOutbox(
  predicate: (s: Awaited<ReturnType<typeof outboxStatus>>) => boolean,
  tries = 20,
): Promise<Awaited<ReturnType<typeof outboxStatus>>> {
  for (let i = 0; i < tries; i++) {
    const s = await outboxStatus();
    if (predicate(s)) return s;
    await new Promise((r) => setTimeout(r, 250));
  }
  throw new Error("outbox status did not reach the expected state in time");
}

beforeAll(() => {
  useRealGateway();
});

describe("rules approval loop (real gateway)", () => {
  it("request_approval raises a needs:approval item and stages the effect HELD", async () => {
    await signInWithCaps("user:ana", WS, CAPS);
    const id = "appr-item-1";

    await runRule({
      body: [
        `inbox.request_approval(#{`,
        `  id: "${id}",`,
        `  channel: "ops",`,
        `  body: "Refund proposed",`,
        `  route: "team:managers",`,
        `  on_approve: #{ target: "notify", action: "page", payload: #{ level: "info" } },`,
        `});`,
      ].join("\n"),
    });

    // The needs:approval item landed on `ops`, tagged, authored as the caller.
    const items = await listInbox("ops");
    const item = items.find((i) => i.id === id);
    expect(item, "the needs:approval item was recorded").toBeTruthy();
    expect(item!.body).toContain("needs:approval");
    expect(item!.body).toContain("route:team:managers");

    // The gated effect is staged HELD — present in the `held` bucket, NOT in pending/delivered.
    const status = await outboxStatus();
    const heldIds = (status.held ?? []).map((e) => e.id);
    expect(heldIds).toContain(`held:${id}`);
    expect(status.pending.map((e) => e.id)).not.toContain(`held:${id}`);
    expect(status.delivered.map((e) => e.id)).not.toContain(`held:${id}`);
  });

  it("approving the item releases the held effect to pending", async () => {
    await signInWithCaps("user:ana", WS, CAPS);
    const id = "appr-item-2";

    await runRule({
      body: [
        `inbox.request_approval(#{`,
        `  id: "${id}", channel: "ops", body: "Refund proposed",`,
        `  on_approve: #{ target: "notify", action: "page", payload: #{} },`,
        `});`,
      ].join("\n"),
    });
    // It starts held.
    const before = await outboxStatus();
    expect((before.held ?? []).map((e) => e.id)).toContain(`held:${id}`);

    // A manager approves through the real resolve verb.
    await resolveInbox(id, "approved");

    // The real reactor tick releases it: held → pending (and out of the held bucket).
    const after = await waitForOutbox((s) =>
      s.pending.some((e) => e.id === `held:${id}`),
    );
    expect((after.held ?? []).map((e) => e.id)).not.toContain(`held:${id}`);
    const released = after.pending.find((e) => e.id === `held:${id}`)!;
    expect(released.status).toBe("pending");
    expect(released.target).toBe("notify");
  });

  it("request_approval is caller-gated: no outbox cap ⇒ denied, opaquely, with no write", async () => {
    // Drop the outbox stage cap; keep inbox.record so we can prove the item never lands either (the
    // effect is staged FIRST, so a deny aborts before the item is recorded — no dangling item).
    await signInWithCaps("user:ana", WS, [
      "mcp:rules.run:call",
      "mcp:inbox.record:call",
      "mcp:inbox.list:call",
      "mcp:outbox.status:call",
    ]);
    const id = "appr-denied";

    await expect(
      runRule({
        body: [
          `inbox.request_approval(#{`,
          `  id: "${id}", channel: "ops", body: "should not land",`,
          `  on_approve: #{ target: "notify", action: "page", payload: #{} },`,
          `});`,
        ].join("\n"),
      }),
    ).rejects.toThrow();

    // No held effect staged, and (because the effect is first) no needs:approval item recorded.
    const status = await outboxStatus();
    expect((status.held ?? []).map((e) => e.id)).not.toContain(`held:${id}`);
    const items = await listInbox("ops");
    expect(items.map((i) => i.id)).not.toContain(id);
  });

  it("the propose-and-approve example in the catalog runs green", async () => {
    // The Examples tab promises its bodies run. Prove the new worked example runs through the real
    // rules.run and stages its held effect.
    await signInWithCaps("user:ana", WS, CAPS);
    const ex = EXAMPLES.find((e) => e.id === "propose-and-approve");
    expect(ex, "the propose-and-approve example exists").toBeTruthy();
    await runRule({ body: ex!.body });

    const status = await outboxStatus();
    expect((status.held ?? []).map((e) => e.id)).toContain("held:refund-proposed");
  });
});
