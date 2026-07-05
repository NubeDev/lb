// The Insights page in the UI (insights umbrella scope), driven against a REAL spawned gateway
// node (no fake — CLAUDE §9 / testing §0). Insights are seeded by raising them through the REAL
// `insight.raise` MCP verb (the same `lb_insights::raise` write the production path uses), then
// listed over the real `GET /insights` route, acked/resolved over the real `POST /insights/{id}/*`
// routes. Each test logs in to a UNIQUE workspace so the shared real node stays isolated.
//
// SKELETON: every test is NAMED for a mandatory or scope-named case + carries the real-gateway
// setup boilerplate. Bodies use `it.todo(...)` so a green-but-lying stub is impossible. The
// implementing session fills them against the scope docs. The harness mirrors
// `InboxView.gateway.test.tsx`.

import { describe, it, beforeAll } from "vitest";

import { InsightsPage } from "./InsightsPage";
import { useRealGateway, signInReal } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `insights-${n++}`;

beforeAll(() => useRealGateway());

/** Raise a real insight via the MCP bridge on the session. */
async function raiseInsight(
  session: { invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> },
  dedupKey: string,
  severity: "info" | "warning" | "critical" = "warning",
): Promise<void> {
  await session.invoke("insight.raise", {
    dedup_key: dedupKey,
    severity,
    title: `test finding ${dedupKey}`,
    origin: { kind: "manual", ref: "test" },
    occurrence: { data: { score: 0.5 } },
    ts: Date.now(),
  });
}

describe("InsightsPage (gateway)", () => {
  it("lists insights raised through the real verb", async () => {
    // SCOPE: insights-scope.md §"MCP surface" — raise → the page's list shows it.
    const ws = nextWs();
    const session = await signInReal("user:ada", ws);
    await raiseInsight(session as never, "k1", "warning");
    void InsightsPage;
    it.todo(
      "assert the raised insight appears in the list with the right severity + count",
    );
  });

  it("ack action resolves through the real route (mandatory cap-deny at the UI layer)", async () => {
    // SCOPE: insights-scope.md §"How it fits the core" → Capabilities. The ack button calls
    // `POST /insights/{id}/ack`; a session without `mcp:insight.ack:call` is refused server-side.
    const ws = nextWs();
    const session = await signInReal("user:ada", ws);
    await raiseInsight(session as never, "k2", "critical");
    it.todo("click Ack, assert the row's status flips to acked");
    it.todo("sign in WITHOUT the ack cap, assert the action is denied server-side");
  });

  it("never shows another workspace's insights (mandatory ws-isolation at the UI layer)", async () => {
    // SCOPE: insights-scope.md §"How it fits the core" → Tenancy/isolation.
    const wsA = nextWs();
    const wsB = nextWs();
    const a = await signInReal("user:ada", wsA);
    await raiseInsight(a as never, "k3", "warning");
    await signInReal("user:bea", wsB);
    it.todo("sign in to ws-B, assert the list is empty (no cross-ws leak)");
  });
});
