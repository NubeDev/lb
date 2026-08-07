// The tag echo on the client side (`docs/scope/insights/insight-tag-echo-scope.md`): dimension
// facets ride BOTH `get` and `list` rows, so a roster renders dimension columns from ONE call.
//
// Against the REAL in-memory client (the package's own transport boundary, seeded with real
// records) and the real `useInsights` hook — no mock of node behaviour (CLAUDE §9). The Rust half
// is pinned by `rust/crates/host/tests/insight_tag_echo_test.rs`; this pins the half that would
// otherwise silently drop the field: the TS `Insight` shape and the list boundary's strip loop.

import { describe, expect, it } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

import type { Insight } from "./types";
import { memoryClient } from "./memoryClient";
import { useInsights } from "./useInsights";

function insight(over: Partial<Insight> = {}): Insight {
  return {
    id: "ins:1",
    dedup_key: "rule:intensity:meter-1",
    severity: "critical",
    title: "intensity above budget",
    origin: { kind: "rule", ref: "intensity" },
    status: "open",
    count: 3,
    first_ts: 1_000,
    last_ts: 2_000,
    producer: "user:test",
    tags: { building: "chullora-dc", asset_type: "water-meter", priority: "medium" },
    evidence: { source: "demo-buildings" },
    ...over,
  };
}

describe("the tag echo on a list row", () => {
  it("survives the boundary that strips evidence — the whole point of the scope", async () => {
    const client = memoryClient([insight()]);
    const { result } = renderHook(() => useInsights(client, {}));
    await waitFor(() => expect(result.current.items.length).toBe(1));

    const row = result.current.items[0];
    // Dimension columns render from the roster response alone: no follow-up get, no `tags.find`.
    expect(row.tags).toEqual({
      building: "chullora-dc",
      asset_type: "water-meter",
      priority: "medium",
    });
    // …while `evidence` is still list-stripped. The two fields sit on opposite sides of the
    // get-vs-list boundary ON PURPOSE ("does the roster render it"), and a future strip loop that
    // treats them alike breaks the roster silently — this is the assertion that catches it.
    expect(row.evidence).toBeUndefined();
  });

  it("is optional — a record raised before the field landed still renders", async () => {
    const { tags: _tags, ...preField } = insight();
    const client = memoryClient([preField as Insight]);
    const { result } = renderHook(() => useInsights(client, {}));
    await waitFor(() => expect(result.current.items.length).toBe(1));
    expect(result.current.items[0].tags).toBeUndefined();
    expect(result.current.items[0].title).toBe("intensity above budget");
  });

  it("is read-only: filtering by a facet is a server-side query, not a client scan of the echo", async () => {
    // A record whose echo is BEHIND the graph (tagged out-of-band, not yet re-raised). A consumer
    // that filtered rows client-side on `tags` would drop it; the correct call ships
    // `ListFilter.tags` to the node, which resolves through the tag graph.
    const stale = insight({ tags: { building: "chullora-dc" } });
    const client = memoryClient([stale]);
    const { result } = renderHook(() =>
      useInsights(client, { tags: { classification: "mechanical" } }),
    );
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.items.map((i) => i.id)).toEqual(["ins:1"]);
  });
});
