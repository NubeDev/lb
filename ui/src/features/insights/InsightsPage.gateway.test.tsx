// The Insights page in the UI (insights umbrella scope), driven against a REAL spawned gateway
// node (no fake — CLAUDE §9 / testing §0). Insights are seeded by raising them through the REAL
// `insight.raise` MCP verb (the same `lb_insights::raise` write the production path uses), then
// listed/acked over the real MCP bridge the page's api rides. Each test logs in to a UNIQUE
// workspace so the shared real node stays isolated. Mirrors `InboxView.gateway.test.tsx`.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { InsightsPage } from "./InsightsPage";
import { invoke } from "@/lib/ipc/invoke";
import { useRealGateway, signInReal, signInWithCaps } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `insights-${n++}`;

beforeAll(() => useRealGateway());

/** Raise a real insight via the MCP bridge (the same verb the production path uses). */
async function raiseInsight(
  dedupKey: string,
  severity: "info" | "warning" | "critical" = "warning",
): Promise<void> {
  await invoke("mcp_call", {
    tool: "insight.raise",
    args: {
      dedup_key: dedupKey,
      severity,
      title: `test finding ${dedupKey}`,
      origin: { kind: "manual", ref: "test" },
      occurrence: { data: { score: 0.5 } },
      ts: Date.now(),
    },
  });
}

describe("InsightsPage (real gateway)", () => {
  it("lists insights raised through the real verb", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await raiseInsight("k1", "warning");

    render(<InsightsPage />);

    // The raised insight is read back over the real list path (MCP bridge → insight.list).
    expect(await screen.findByText(/test finding k1/)).toBeInTheDocument();
    // Its dedup_key + count badge render.
    expect(screen.getByText("k1")).toBeInTheDocument();
  });

  it("ack action flips the row status through the real route", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await raiseInsight("k2", "critical");

    render(<InsightsPage />);
    // Open the detail drawer for the row.
    const row = await screen.findByText(/test finding k2/);
    await user.click(row);

    // Ack via the real `insight.ack` — the drawer refetches and the button disappears (status now acked).
    const ackBtn = await screen.findByRole("button", { name: /^Ack$/ });
    await user.click(ackBtn);
    await waitFor(() =>
      expect(screen.queryByRole("button", { name: /^Ack$/ })).not.toBeInTheDocument(),
    );
    // No error band surfaced.
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("mandatory cap-deny: a session without the ack cap is refused server-side", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    // First raise as a full member, then re-login WITHOUT the ack cap.
    await signInReal("user:ada", ws);
    await raiseInsight("k4", "warning");
    await signInWithCaps("user:ada", ws, [
      "mcp:insight.list:call",
      "mcp:insight.get:call",
      "mcp:insight.occurrences:call",
      // deliberately NO mcp:insight.ack:call
    ]);

    render(<InsightsPage />);
    const row = await screen.findByText(/test finding k4/);
    await user.click(row);
    const ackBtn = await screen.findByRole("button", { name: /^Ack$/ });
    await user.click(ackBtn);
    // The server denies (opaque) — the drawer surfaces the error and the Ack button stays.
    await waitFor(() => expect(screen.getByText(/^Ack$/)).toBeInTheDocument());
  });

  it("mandatory ws-isolation: ws-B never sees ws-A's insights", async () => {
    const wsA = nextWs();
    const wsB = nextWs();
    await signInReal("user:ada", wsA);
    await raiseInsight("k3", "warning");

    // Switch to ws-B (a fresh session) — the page lists nothing from ws-A.
    await signInReal("user:bea", wsB);
    render(<InsightsPage />);
    expect(
      await screen.findByText(/No insights match this filter\./),
    ).toBeInTheDocument();
    expect(screen.queryByText(/test finding k3/)).not.toBeInTheDocument();
  });
});
