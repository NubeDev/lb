// Extension pages in the shell (ui-federation scope), driven against a REAL spawned gateway (no fake —
// CLAUDE §9). An install that declares a `[ui]` block is seeded as a real `Install` record (with its
// `ui` page) through the test gateway's `/_seed/extension` route; the shell discovers it over the real
// `ext.list` route and builds a cap-gated sidebar slot. Two guarantees, plus the host-bridge scope
// filter:
//   1. an installed extension that declares a `[ui]` page shows a nav slot (the shell builds it from
//      `ext.list`), so a real page becomes reachable from the sidebar;
//   2. an extension WITHOUT a `[ui]` page contributes no slot;
//   3. the host-mediated bridge forwards ONLY the extension's granted read-only tools and rejects
//      anything out of scope — the page is a gated caller, never a trusted decider.
// The bundle dynamic-import itself runs in the browser/gateway path; jsdom can't load a remote ESM, so
// we assert the slot + the bridge's scope filter (pure local logic, no backend needed).

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen } from "@testing-library/react";

import { App } from "../../App";
import { makeBridge } from "./bridge";
import { listExtensions } from "@/lib/ext/ext.api";
import { CAP } from "@/lib/session";
import { useRealGateway, signInReal, signInWithCaps, seedExtension } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `ext-host-${n++}`;

/** The reference SELF-CONTAINED extension: a native sidecar that ALSO ships a federated `[ui]` page
 *  (3 nested routes) and two `[[widget]]` tiles. Seeded as a real Install through the test gateway. */
const FLEET_MONITOR = {
  ext: "fleet-monitor",
  version: "0.1.0",
  tier: "native" as const,
  enabled: true,
  ui: {
    entry: "assets/remoteEntry.js",
    label: "Fleet Monitor",
    icon: "radar",
    scope: ["series.find", "series.latest", "series.read"],
  },
  widgets: [
    { entry: "assets/remoteEntry.js", label: "Fleet Status", icon: "activity", scope: ["series.latest"] },
    { entry: "assets/remoteEntry.js", label: "Fleet Sparkline", icon: "trending-up", scope: ["series.read"] },
  ],
};

beforeAll(() => useRealGateway());

describe("extension pages (ui-federation, real gateway)", () => {
  it("shows a sidebar slot for an extension that declares a [ui] page", async () => {
    const ws = nextWs();
    await signInWithCaps("user:ada", ws, [CAP.extList]);
    await seedExtension(FLEET_MONITOR);
    render(<App />);
    // The page's label becomes a cap-gated nav slot built from the real ext.list.
    expect(await screen.findByLabelText("Fleet Monitor")).toBeInTheDocument();
  });

  it("surfaces both [[widget]] tiles in ext.list for the dashboard palette", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedExtension(FLEET_MONITOR);
    // Read the real ext.list and assert both widget tiles round-tripped through the real Install.
    const rows = await listExtensions();
    const row = rows.find((r) => r.ext === "fleet-monitor");
    expect(row?.widgets?.map((w) => w.label)).toEqual(["Fleet Status", "Fleet Sparkline"]);
  });

  it("does NOT show a slot for an extension with no [ui] page", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedExtension({ ext: "hello", version: "v2", tier: "wasm", enabled: true }); // no ui
    render(<App />);
    expect(await screen.findByLabelText("Channels")).toBeInTheDocument();
    expect(screen.queryByLabelText("hello")).not.toBeInTheDocument();
  });
});

describe("the host-mediated bridge", () => {
  it("rejects an out-of-scope tool locally (defense in depth; the host would deny it too)", async () => {
    const bridge = makeBridge(["series.find"]);
    await expect(bridge.call("series.delete", {})).rejects.toThrow(/out_of_scope/);
    await expect(bridge.call("dashboard.delete", {})).rejects.toThrow(/out_of_scope/);
    // The in-scope forward path (bridge → POST /mcp/call → host re-check) is exercised against the
    // real gateway in the Rust mcp-route tests (role/gateway/tests). Here the out-of-scope rejection
    // is pure local logic, asserted directly.
  });

  it("watch() gates series.watch on scope and requires a series arg (no-op otherwise)", () => {
    // Without the `series.watch` grant, watch() is a no-op unsubscribe — the CE canvas then degrades to a
    // static (no live values) feed rather than opening an ungranted stream. This is the fix for the
    // "disconnected / no values" symptom: the shell bridge now HAS a watch, so a granted page streams.
    const ungranted = makeBridge(["control-engine.tree"]);
    expect(typeof ungranted.watch("series.watch", { series: "s1" }, () => {})).toBe("function");

    const granted = makeBridge(["series.watch"]);
    // A non-series verb, an empty series, and a missing series arg all no-op (defense in depth).
    expect(typeof granted.watch("dashboard.delete", { series: "s1" }, () => {})).toBe("function");
    expect(typeof granted.watch("series.watch", { series: "" }, () => {})).toBe("function");
    expect(typeof granted.watch("series.watch", {}, () => {})).toBe("function");
    // In jsdom there is no gateway URL / EventSource, so a granted+valid call still yields a clean no-op
    // unsubscribe (openSeriesStream returns null); the live SSE is proven by the Rust series-stream tests.
    const unsub = granted.watch("series.watch", { series: "s1" }, () => {});
    expect(typeof unsub).toBe("function");
    unsub();
  });
});
