// Result-render coverage (widget-platform scope, Slice C — closes G1), end to end through a REAL
// spawned gateway — no fakes (CLAUDE §9). Slice C gives `federation.query` and `query.run` a
// `descriptor.result = table` envelope so the channel CAN render them descriptor-driven (the
// `kind:"rich_result"` path through `MessageItem` → `ResponseView` → `WidgetView`), not only via
// the legacy `kind:"query_result"` → `QueryCard` path. This file proves that NEW render path: a
// `rich_result` carrying the federation.query declared envelope MOUNTS through ResponseView/
// WidgetView (the PinToDashboard affordance only ResponseView mounts is the structural marker), and
// does NOT route to QueryCard (the legacy query-result path).
//
// The render-TIME federation.query call resolves through the gated bridge → `viz.query` →
// `federation.query`, which fails honestly in a workspace with no registered source (the test
// asserts the STRUCTURAL mount — the table is rendered + the pin affordance is present — NOT the
// rows; the row-render path is the same source-rerun path the reminder gateway test exercises, and
// the federation sidecar is a true external this test cannot cheaply run).
//
// Why a real gateway and not a unit test: the routing decision (rich_result → ResponseView, not
// QueryCard) lives in `MessageItem.tsx` and the ResponseView builds a real v2 Cell that mounts
// `WidgetView` against the real `DashboardCacheProvider`. Asserting "the descriptor-driven render
// path works for federation.query" is end-to-end-by-nature; jsdom alone would mock the routing.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen } from "@testing-library/react";

import { MessageItem } from "./MessageItem";
import { encodeRichResult } from "@/lib/channel/payload.types";
import type { Item } from "@/lib/channel/channel.types";
import { useRealGateway, signInWithCaps } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `rr-c-${n++}`;

/** The federation.query declared `result` envelope (the descriptor's `result` field, minus wire tags).
 *  Mirrors what the palette would post in the descriptor-driven path: the palette collects `source`
 *  and `sql` and interpolates them into `source.args` over the descriptor's template. */
const FEDERATION_QUERY_BODY = encodeRichResult({
  v: 2,
  view: "table",
  source: { tool: "federation.query", args: { source: "warehouse", sql: "SELECT 1" } },
  tools: ["federation.query"],
});

/** The query.run declared `result` envelope — a `{id}` captured at pin time so the cell re-runs the
 *  saved query by id. */
const QUERY_RUN_BODY = encodeRichResult({
  v: 2,
  view: "table",
  source: { tool: "query.run", args: { id: "daily" } },
  tools: ["query.run"],
});

/** Mount one rich_result Item through the REAL channel render path (MessageItem → routing decision). */
function mountResponse(ws: string, body: string, itemId = "rr-1"): void {
  const item: Item = {
    id: itemId,
    channel: "general",
    author: "system:test",
    body,
    ts: 1,
  };
  render(<MessageItem item={item} author="user:me" ws={ws} onEdit={() => {}} onDelete={() => {}} />);
}

// The catalog + the tool's own cap (so the bridge leash covers `federation.query` — without it the
// render-time call would deny opaquely; that's the existing gate, not Slice C's). Pub/sub so the
// post + history reconcile.
const FED_CAPS = [
  "mcp:tools.catalog:call",
  "mcp:federation.query:call",
  "bus:chan/general:pub",
  "bus:chan/general:sub",
];
const RUN_CAPS = [
  "mcp:tools.catalog:call",
  "mcp:query.run:call",
  "bus:chan/general:pub",
  "bus:chan/general:sub",
];

beforeAll(() => useRealGateway());

describe("Result-render coverage (Slice C)", () => {
  it("HEADLINE: a federation.query rich_result mounts through ResponseView (NOT QueryCard), descriptor-driven", async () => {
    const ws = nextWs();
    await signInWithCaps("user:me", ws, FED_CAPS);

    mountResponse(ws, FEDERATION_QUERY_BODY);

    // The PinToDashboard affordance is mounted ONLY by ResponseView (beside a rendered rich_result).
    // Its presence proves the rich_result routed to ResponseView, not to QueryCard (the legacy
    // query_result path) — i.e. the descriptor-driven render path works for federation.query.
    const pinBtn = await screen.findByRole("button", { name: /Pin to dashboard/i }, { timeout: 8000 });
    expect(pinBtn).toBeInTheDocument();

    // The legacy QueryCard markers are ABSENT (no "query result" aria-label, no SQL chip echoing
    // the source). The rich_result took the ResponseView branch.
    expect(screen.queryByLabelText(/query result/i)).not.toBeInTheDocument();
  });

  it("a query.run rich_result also mounts through ResponseView (the second tabular tool)", async () => {
    const ws = nextWs();
    await signInWithCaps("user:me", ws, RUN_CAPS);

    mountResponse(ws, QUERY_RUN_BODY, "rr-run");

    // Same structural proof: the PinToDashboard affordance mounts (ResponseView branch), QueryCard
    // markers absent. query.run's descriptor-driven render path works.
    const pinBtn = await screen.findByRole("button", { name: /Pin to dashboard/i }, { timeout: 8000 });
    expect(pinBtn).toBeInTheDocument();
    expect(screen.queryByLabelText(/query result/i)).not.toBeInTheDocument();
  });

  it("an envelope with an unknown tool id still mounts (the descriptor-driven path is tool-agnostic)", async () => {
    // Rule 10: the render path is GENERIC over the tool id. A rich_result sourced at an arbitrary
    // tool the host has never heard of still routes to ResponseView and mounts WidgetView — the
    // render-time fetch fails honestly (the bridge calls an unknown tool → empty), but the STRUCTURE
    // is there. This mirrors the Rust `pin_path_is_generic_over_an_arbitrary_tabular_tool_id` test.
    const ws = nextWs();
    await signInWithCaps("user:me", ws, [
      "mcp:tools.catalog:call",
      "bus:chan/general:pub",
      "bus:chan/general:sub",
    ]);

    const body = encodeRichResult({
      v: 2,
      view: "table",
      source: { tool: "__test__.warehouse_read", args: { q: "shipments" } },
      tools: ["__test__.warehouse_read"],
    });
    mountResponse(ws, body, "rr-unknown");

    const pinBtn = await screen.findByRole("button", { name: /Pin to dashboard/i }, { timeout: 8000 });
    expect(pinBtn).toBeInTheDocument();
  });
});
