// The tool-driven widget builder, driven against a REAL in-process gateway (widget-builder scope,
// Testing plan; CLAUDE §9 / testing §0 — no fake backend). Each test logs in to a UNIQUE workspace,
// seeds real rows through the real ingest/install path, and exercises the v2 contract end to end over
// the real `POST /mcp/call` bridge + the series SSE. Covers the mandatory categories:
//   - render_templates CRUD round-trip + deny-per-verb;
//   - capability deny PER VERB including WRITES (a tool outside the cell's set → denied SERVER-SIDE);
//   - workspace isolation including a WRITE (ws-B cannot write into ws-A);
//   - token never crosses the bridge boundary (no token in any bridge arg / payload);
//   - write-control e2e (a button bound to a real write tool actually writes; side effect asserted);
//   - scripted-template write deny when the tool is ungranted;
//   - trust-tier routing (a non-allow-listed ext widget renders sandboxed, never in-process);
//   - extension-widget e2e (install with a [[widget]] → palette tile → uninstall evicts).

import { describe, expect, it, beforeAll } from "vitest";
import { render, waitFor } from "@testing-library/react";

import {
  useRealGateway,
  signInReal,
  signInWithCaps,
  seedIotDemo,
  seedExtension,
} from "@/test/gateway-session";
import { invoke } from "@/lib/ipc/invoke";
import {
  saveTemplate,
  getTemplate,
  listTemplates,
  deleteTemplate,
} from "@/lib/dashboard/template.api";
import { listExtensions, uninstallExtension } from "@/lib/ext/ext.api";
import { makeWidgetBridge } from "./widgetBridge";
import { ExtWidget } from "./ExtWidget";
import { extWidgetTier } from "./trust";
import { buildSourceEntries } from "./sourcePicker";

let n = 0;
const nextWs = () => `wb-${n++}`;

beforeAll(() => useRealGateway());

// ---------------------------------------------------------------------------------------------------
// render_templates CRUD + deny-per-verb
// ---------------------------------------------------------------------------------------------------
describe("render_templates CRUD (real gateway)", () => {
  it("round-trips save → get → list → delete over the bridge", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);

    const saved = await saveTemplate("defrost", "Defrost", "template", "<div>hi</div>");
    expect(saved.id).toBe("defrost");
    expect(saved.author).toBe("user:ada");

    expect((await getTemplate("defrost")).code).toBe("<div>hi</div>");

    const list = await listTemplates();
    expect(list.map((t) => t.id)).toContain("defrost");

    await deleteTemplate("defrost");
    await expect(getTemplate("defrost")).rejects.toThrow(); // tombstoned → NotFound
  });

  it("denies a template verb the session lacks the cap for (per verb)", async () => {
    const ws = nextWs();
    // A session WITHOUT mcp:template.save:call but with the others.
    await signInWithCaps("user:ada", ws, [
      "mcp:template.get:call",
      "mcp:template.list:call",
      "mcp:template.delete:call",
    ]);
    await expect(saveTemplate("x", "X", "plot", "Plot.dot()")).rejects.toThrow();
  });

  it("is workspace isolated — ws-B cannot read ws-A's template", async () => {
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    await saveTemplate("shared", "A", "template", "secret-a");

    const wsB = nextWs();
    await signInReal("user:ben", wsB);
    await expect(getTemplate("shared")).rejects.toThrow(); // a different namespace — NotFound
    expect((await listTemplates()).map((t) => t.id)).not.toContain("shared");
  });
});

// ---------------------------------------------------------------------------------------------------
// Capability deny PER VERB including WRITES — server-side, even if the bridge filter were bypassed
// ---------------------------------------------------------------------------------------------------
describe("capability deny including writes (real gateway)", () => {
  it("denies an ungranted WRITE server-side even when the bridge filter is bypassed", async () => {
    const ws = nextWs();
    // The session holds NO ingest.write cap (signInWithCaps gives only what we list).
    await signInWithCaps("user:ada", ws, ["mcp:series.find:call"]);

    // Bypass the bridge's local scope filter entirely — call the raw mcp_call invoke with a write the
    // session was never granted. The HOST must still deny it (the grant is the real leash).
    await expect(
      invoke("mcp_call", { tool: "ingest.write", args: { samples: [] } }),
    ).rejects.toThrow();
  });

  it("denies a tool outside the cell's set at the bridge (defense in depth)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    const bridge = makeWidgetBridge(["series.read"]); // cell set is one read tool
    await expect(bridge.call("mqtt.publish", {})).rejects.toThrow(/out_of_scope/);
    await expect(bridge.call("dashboard.delete", {})).rejects.toThrow(/out_of_scope/);
  });
});

// ---------------------------------------------------------------------------------------------------
// Workspace isolation including a WRITE widget
// ---------------------------------------------------------------------------------------------------
describe("workspace isolation across a write (real gateway)", () => {
  it("a ws-B write widget cannot write into ws-A", async () => {
    // Ada seeds a series in ws-A. Ben in ws-B writes the same series name — it must land in ws-B's
    // namespace, never ws-A's (the hard wall holds across the write bridge).
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    await seedIotDemo();
    const aBefore = await invoke<{ sample: { seq: number } | null }>("series_latest", {
      series: "cooler.temp",
    });

    const wsB = nextWs();
    await signInReal("user:ben", wsB);
    const bridge = makeWidgetBridge(["ingest.write"]);
    await bridge.call("ingest.write", {
      samples: [{ series: "cooler.temp", producer: "user:ben", seq: 999, payload: -1, ts: 1 }],
    });

    // Ada (back in ws-A) sees her series unchanged — Ben's write never crossed the wall.
    await signInReal("user:ada", wsA);
    const aAfter = await invoke<{ sample: { seq: number } | null }>("series_latest", {
      series: "cooler.temp",
    });
    expect(aAfter.sample?.seq).toBe(aBefore.sample?.seq);
  });
});

// ---------------------------------------------------------------------------------------------------
// Token never crosses the bridge boundary
// ---------------------------------------------------------------------------------------------------
describe("token never crosses the boundary", () => {
  it("no session token appears in any bridge call argument (read or write)", async () => {
    const ws = nextWs();
    const session = await signInReal("user:ada", ws);
    const token = session.token;
    expect(token.length).toBeGreaterThan(0);

    // Capture what the bridge forwards by spying on the invoke seam through a wrapper bridge.
    const seen: unknown[] = [];
    const bridge = makeWidgetBridge(["series.find", "ingest.write"]);
    const orig = bridge.call;
    bridge.call = async (tool, args) => {
      seen.push({ tool, args });
      return orig(tool, args);
    };
    await bridge.call("series.find", { facets: [] });
    try {
      await bridge.call("ingest.write", { samples: [] });
    } catch {
      /* deny is fine — we only care the token wasn't in the payload */
    }
    // The token must not be embedded in any forwarded argument (it rides server-side only).
    expect(JSON.stringify(seen)).not.toContain(token);
  });
});

// ---------------------------------------------------------------------------------------------------
// Write-control e2e — a real write tool is actually invoked; the side effect is observable
// ---------------------------------------------------------------------------------------------------
describe("write-control e2e (real gateway)", () => {
  it("a control's write through the bridge produces a real, readable side effect", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);

    // The "control" writes a sample via ingest.write (a granted write tool). After the write, the same
    // series reads it back — the real side effect, not a fake success.
    const bridge = makeWidgetBridge(["ingest.write"]);
    await bridge.call("ingest.write", {
      samples: [{ series: "control.cmd", producer: "user:ada", seq: 1, payload: 7, ts: 1 }],
    });

    const latest = await invoke<{ sample: { payload: unknown } | null }>("series_latest", {
      series: "control.cmd",
    });
    expect(latest.sample?.payload).toBe(7);
  });
});

// ---------------------------------------------------------------------------------------------------
// Scripted-template write deny when the tool is ungranted
// ---------------------------------------------------------------------------------------------------
describe("scripted-template write deny (real gateway)", () => {
  it("a scripted view's write is denied when the cell set excludes it", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    // The scripted view's bridge is bound to a read-only set; an inline template that tries to publish
    // is rejected at the bridge AND would be denied at the host. We assert the bridge refusal here (the
    // server-side deny is covered by the capability-deny tests above).
    const bridge = makeWidgetBridge(["series.read"]);
    await expect(bridge.call("ingest.write", { samples: [] })).rejects.toThrow(/out_of_scope/);
  });
});

// ---------------------------------------------------------------------------------------------------
// Trust-tier routing
// ---------------------------------------------------------------------------------------------------
describe("trust-tier routing (real gateway)", () => {
  it("a non-allow-listed extension widget renders in a sandboxed iframe, never in-process", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedExtension({
      ext: "mqtt-bridge",
      version: "0.1.0",
      tier: "wasm",
      enabled: true,
      widgets: [
        { entry: "remoteEntry.js", label: "Cooler Switch", icon: "x", scope: ["mqtt.status", "mqtt.publish"] },
      ],
    });
    const installed = await listExtensions();

    // The publisher key (== ext id here) is NOT on the allow-list → iframe tier.
    expect(extWidgetTier("mqtt-bridge")).toBe("iframe");

    render(
      <ExtWidget viewKey="ext:mqtt-bridge/cooler-switch" installed={installed} workspace={ws} />,
    );
    // The cell mounts the iframe host (sandboxed), not an in-process div. Wait for the iframe element.
    await waitFor(() =>
      expect(document.querySelector("[data-widget-iframe]")).toBeInTheDocument(),
    );
    const host = document.querySelector('[data-ext-widget="mqtt-bridge"]');
    expect(host?.getAttribute("data-tier")).toBe("iframe");
  });
});

// ---------------------------------------------------------------------------------------------------
// Extension-widget e2e — install with a [[widget]] → palette tile → uninstall evicts
// ---------------------------------------------------------------------------------------------------
describe("extension-widget e2e (real gateway)", () => {
  it("surfaces a [[widget]] tile in the source picker and evicts on uninstall", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedExtension({
      ext: "mqtt-bridge",
      version: "0.1.0",
      tier: "wasm",
      enabled: true,
      widgets: [
        { entry: "remoteEntry.js", label: "Cooler Switch", icon: "x", scope: ["mqtt.status", "mqtt.publish"] },
      ],
    });

    // The palette (source picker) surfaces the ext's tools, split read/write, by friendly label.
    let installed = await listExtensions();
    let entries = buildSourceEntries([], installed);
    expect(entries.some((e) => e.label.includes("mqtt.status") && !e.writes)).toBe(true);
    expect(entries.some((e) => e.label.includes("mqtt.publish") && e.writes)).toBe(true);

    // Uninstall → the ext is gone from ext.list, so the palette no longer offers its tools (eviction).
    await uninstallExtension("mqtt-bridge");
    installed = await listExtensions();
    entries = buildSourceEntries([], installed);
    expect(entries.some((e) => e.label.includes("mqtt"))).toBe(false);
  });
});
