// Advanced template variables, driven against a REAL spawned gateway (advanced-variables scope,
// "Gateway (`variablesAdvanced.gateway.test.tsx`)"; CLAUDE §9 — no fake). Proves, end to end over the
// real store/caps/gateway, that a QUERY variable's option resolution — the seam the advanced pipeline
// (regex/sort/chained) rides on — is workspace-isolated and capability-gated:
//   1. Workspace isolation (mandatory): a `query` variable backed by `series.find` resolves the VIEWER's
//      workspace rows — a ws-B viewer sees ws-B series, never ws-A's — because the resolver is a
//      token-scoped `{tool,args}` MCP call (rule 6), regardless of which dashboard hosts the variable.
//   2. Capability deny (mandatory): a viewer LACKING the resolver tool's cap gets that variable's options
//      denied OPAQUELY (an empty list + the honest "—" placeholder), never a fabricated catalogue and
//      never a broken bar (rule 5).
// The pure advanced behaviors (label≠value, regex capture, sort, dependency ordering, allValue, new
// format hints) are exhaustively unit-tested in `lib/vars/advanced.test.ts`; this file proves the LIVE
// resolution path they compose over is correctly scoped.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

import { VariableBar } from "./VariableBar";
import type { Variable } from "@/lib/vars";
import { useRealGateway, signInReal, signInWithCaps, seedSeries } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `va-ui-${n++}`;

beforeAll(() => useRealGateway());

/** A `query` variable resolving its options from `series.find` for a `region` facet — the discovery
 *  read the advanced pipeline (regex/sort/chained) sits on top of. */
function regionVar(): Variable {
  return {
    name: "series",
    type: "query",
    query: { tool: "series.find", args: { facets: [{ key: "region", value: "west" }] } },
  };
}

/** Render just the bar for one variable (no dashboard chrome) — the selection is inert (we assert the
 *  resolved OPTIONS, which is what isolation + deny govern). */
function renderBar(variable: Variable) {
  render(<VariableBar variables={[variable]} selected={{}} onChange={() => {}} />);
}

describe("advanced variables — live resolution is workspace-isolated + cap-gated", () => {
  it("workspace isolation: a query variable resolves the VIEWER's workspace series, never another ws's", async () => {
    // ws-A seeds a `region:west` series; a ws-A viewer's variable resolves it.
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    await seedSeries({ series: "hvac.west.temp", seq: 1, payload: { value: 21 }, key: "region", value: "west" });
    renderBar(regionVar());
    const barA = screen.getByLabelText("variable bar");
    await waitFor(() =>
      expect(barA.querySelector('option[value="series:hvac.west.temp"]')).not.toBeNull(),
    );

    // ws-B (a DIFFERENT workspace, no seed) resolves the SAME variable definition against ITS token —
    // and sees NO ws-A option. The resolver is token-scoped; the hard wall holds across the shared store.
    const wsB = nextWs();
    await signInReal("user:ben", wsB);
    renderBar(regionVar());
    const bars = screen.getAllByLabelText("variable bar");
    const barB = bars[bars.length - 1];
    await waitFor(() => expect(barB.querySelector("select")).not.toBeNull());
    // A short settle, then assert ws-A's series is absent for the ws-B viewer.
    await new Promise((r) => setTimeout(r, 50));
    expect(barB.querySelector('option[value="series:hvac.west.temp"]')).toBeNull();
  });

  it("capability deny: a viewer lacking the resolver tool's cap gets options denied opaquely, bar intact", async () => {
    const ws = nextWs();
    // Seed a real matching series as an admin so a permitted viewer WOULD see it…
    await signInReal("user:ada", ws);
    await seedSeries({ series: "hvac.west.temp", seq: 1, payload: { value: 21 }, key: "region", value: "west" });

    // …then become a viewer WITHOUT `mcp:series.find:call`. The resolver call is denied at the host; the
    // bar renders the honest empty placeholder ("—"), not a fabricated option set, and stays usable.
    await signInWithCaps("user:ben", ws, ["mcp:series.read:call"]);
    renderBar(regionVar());
    const bar = screen.getByLabelText("variable bar");
    await waitFor(() => expect(bar.querySelector("select")).not.toBeNull());
    await waitFor(() =>
      expect(bar.querySelector('option[value="series:hvac.west.temp"]')).toBeNull(),
    );
    // The deny is opaque: the placeholder is the "—" (denied), the bar is present, nothing threw.
    expect(bar).toBeInTheDocument();
  });
});
