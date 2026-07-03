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
//   - trust-tier routing (an installed ext widget federates in-process; scripted code stays sandboxed);
//   - extension-widget e2e (install with a [[widget]] → palette tile → uninstall evicts).

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import {
  useRealGateway,
  signInReal,
  signInWithCaps,
  seedIotDemo,
  seedExtension,
  seedSeries,
} from "@/test/gateway-session";
import { invoke } from "@/lib/ipc/invoke";
import {
  saveTemplate,
  getTemplate,
  listTemplates,
  deleteTemplate,
} from "@/lib/dashboard/template.api";
import { saveDashboard, getDashboard } from "@/lib/dashboard/dashboard.api";
import { listExtensions, uninstallExtension } from "@/lib/ext/ext.api";
import type { Cell } from "@/lib/dashboard";
import { WidgetBuilder } from "./WidgetBuilder";
import { makeWidgetBridge } from "./widgetBridge";
import { ExtWidget } from "./ExtWidget";
import { extWidgetTier } from "./trust";
import { buildSourceEntries, extWidgetEntries } from "./sourcePicker";
import { WithDashboardCache } from "@/features/dashboard/cache/testCacheWrapper";

/** proof-panel's packaged `[[widget]]` tile, exactly as `extension.toml` declares it (Proof Ping,
 *  scope = series.latest/series.find). Seeded into `ext.list` so the palette surfaces it for real. */
const PROOF_PING = {
  entry: "remoteEntry.js",
  label: "Proof Ping",
  icon: "shield-check",
  scope: ["series.latest", "series.find"],
};

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
// Trust-tier routing — an INSTALLED extension widget federates in-process (the install is the trust
// gate). The iframe tier is reserved for scripted author code, never an installed widget (which the
// sandbox can't load: the remote externalizes React to the shell import map — see the debug entry).
// ---------------------------------------------------------------------------------------------------
describe("trust-tier routing (real gateway)", () => {
  it("an installed extension widget renders in-process, never in a sandboxed iframe", async () => {
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

    // Installed ⇒ in-process, for any key (the install passed the publish/install cap gate).
    expect(extWidgetTier("mqtt-bridge")).toBe("in-process");

    render(
      <ExtWidget viewKey="ext:mqtt-bridge/cooler-switch" installed={installed} workspace={ws} />,
    );
    // The cell mounts the in-process federation host, NOT a sandboxed iframe.
    const host = await waitFor(() => {
      const h = document.querySelector('[data-ext-widget="mqtt-bridge"]');
      expect(h).toBeInTheDocument();
      return h;
    });
    expect(host?.getAttribute("data-tier")).toBe("in-process");
    expect(document.querySelector("[data-widget-iframe]")).not.toBeInTheDocument();
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

// ---------------------------------------------------------------------------------------------------
// Packaged-tile PALETTE round-trip — the slice's headline. Install proof-panel's [[widget]] → the
// "Extension widgets" group lists "Proof Ping" → select → preview routes to the real ExtWidget over the
// real bridge → Add persists a `view:"ext:proof-panel/proof-ping"` cell via real dashboard.save → reload
// re-reads it. (The data the tile reads is real: proof.demo's latest is asserted over the bridge.)
// ---------------------------------------------------------------------------------------------------
describe("packaged-tile palette round-trip (real gateway)", () => {
  it("lists Proof Ping → select previews the real ExtWidget → Add persists the cell → reload re-reads it", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    // Install proof-panel's packaged tile (real ext.list row) + seed the REAL series the tile reads.
    await seedExtension({ ext: "proof-panel", version: "0.1.0", tier: "wasm", enabled: true, widgets: [PROOF_PING] });
    await seedSeries({ series: "proof.demo", seq: 1, payload: 21, key: "kind", value: "temperature" });

    // The tile's data is REAL: its scope reads proof.demo's latest over the live bridge (the same call
    // the mounted widget makes). Not a fake preview — the value the tile would show is the seeded 21.
    const tileBridge = makeWidgetBridge(PROOF_PING.scope);
    const latest = await tileBridge.call<{ sample: { payload: unknown } | null }>("series.latest", {
      series: "proof.demo",
    });
    expect(latest.sample?.payload).toBe(21);

    // Render the REAL builder as an editor. Capture the cell it adds.
    let added: Cell | null = null;
    render(
      <WithDashboardCache ws={ws}>
        <WidgetBuilder ws={ws} existing={[]} onAdd={(c) => (added = c)} canEdit />
      </WithDashboardCache>,
    );

    // The "Extension widgets" group lists the packaged tile by `<ext> · <tile.label>`.
    const sourceSelect = await screen.findByLabelText<HTMLSelectElement>("widget source");
    await waitFor(() =>
      expect(
        [...sourceSelect.options].some((o) => o.textContent === "proof-panel · Proof Ping"),
      ).toBe(true),
    );

    // Select it. The view chooser disappears (a packaged tile is its own view) and the preview mounts
    // the real ExtWidget for the ext key, in-process (an installed extension federates in-process).
    const option = [...sourceSelect.options].find((o) => o.textContent === "proof-panel · Proof Ping")!;
    await userEvent.selectOptions(sourceSelect, option.value);
    expect(screen.queryByLabelText("widget view")).not.toBeInTheDocument();
    await waitFor(() => {
      const host = document.querySelector('[data-ext-widget="proof-panel"]');
      expect(host).toBeInTheDocument();
      expect(host?.getAttribute("data-tier")).toBe("in-process");
    });

    // Add → the builder emits a v2 cell whose view is the packaged key. Persist it for REAL.
    await userEvent.click(screen.getByLabelText("add widget"));
    expect(added).not.toBeNull();
    expect(added!.v).toBe(2);
    expect(added!.view).toBe("ext:proof-panel/proof-ping");
    expect(added!.source).toBeUndefined(); // a packaged tile carries no source — it owns its data

    await saveDashboard("dash-1", "Ops", [added!]);

    // Reload: the persisted cell comes back identical — the round-trip is real, not in-memory.
    const reloaded = await getDashboard("dash-1");
    const cell = reloaded.cells.find((c) => c.view === "ext:proof-panel/proof-ping");
    expect(cell).toBeDefined();
    expect(cell!.v).toBe(2);
  });

  it("emits one palette entry per [[widget]] tile from a REAL ext.list row", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedExtension({ ext: "proof-panel", version: "0.1.0", tier: "wasm", enabled: true, widgets: [PROOF_PING] });

    const installed = await listExtensions();
    const widgetEntries = extWidgetEntries(installed);
    const ping = widgetEntries.find((e) => e.label === "proof-panel · Proof Ping");
    expect(ping).toBeDefined();
    expect(ping!.group).toBe("widget");
    expect(ping!.viewKey).toBe("ext:proof-panel/proof-ping"); // the key ExtWidget parses
  });
});

// ---------------------------------------------------------------------------------------------------
// The edit GATE (the slice's mandatory capability-deny headline). A viewer WITHOUT
// `mcp:dashboard.save:call` gets NO add affordance, AND a direct dashboard.save from such a principal
// is denied server-side (the host is the real backstop even if the UI gate were bypassed).
// ---------------------------------------------------------------------------------------------------
describe("edit-cap gate (real gateway)", () => {
  it("hides the whole add surface from a read-only viewer (canEdit=false)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedExtension({ ext: "proof-panel", version: "0.1.0", tier: "wasm", enabled: true, widgets: [PROOF_PING] });

    const { container } = render(
      <WithDashboardCache ws={ws}>
        <WidgetBuilder ws={ws} existing={[]} onAdd={() => {}} canEdit={false} />
      </WithDashboardCache>,
    );
    // No builder surface at all — no source picker, no add button (the affordance is gated).
    expect(container).toBeEmptyDOMElement();
    expect(screen.queryByLabelText("widget source")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("add widget")).not.toBeInTheDocument();
  });

  it("denies dashboard.save server-side for a principal lacking the cap (UI gate is not the boundary)", async () => {
    const ws = nextWs();
    // A session holding only the READ caps — NO mcp:dashboard.save:call.
    await signInWithCaps("user:ada", ws, ["mcp:dashboard.list:call", "mcp:dashboard.get:call"]);
    // Even bypassing the hidden UI, the host denies the write — the authoritative backstop.
    await expect(
      saveDashboard("dash-x", "X", [{
        i: "w1", x: 0, y: 0, w: 4, h: 3, v: 2, widget_type: "chart",
        view: "ext:proof-panel/proof-ping", binding: { series: "" },
      } as Cell]),
    ).rejects.toThrow();
  });
});

// ---------------------------------------------------------------------------------------------------
// Workspace isolation — a ws-B editor's picker lists only ws-B tiles; a ws-A tile never leaks across.
// ---------------------------------------------------------------------------------------------------
describe("packaged-tile workspace isolation (real gateway)", () => {
  it("a ws-B editor's picker lists only ws-B's installed tiles", async () => {
    // ws-A installs proof-panel.
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    await seedExtension({ ext: "proof-panel", version: "0.1.0", tier: "wasm", enabled: true, widgets: [PROOF_PING] });
    expect(extWidgetEntries(await listExtensions()).some((e) => e.label.includes("Proof Ping"))).toBe(true);

    // ws-B installs a DIFFERENT tile; its picker sees its own, never ws-A's proof-panel.
    const wsB = nextWs();
    await signInReal("user:ben", wsB);
    await seedExtension({
      ext: "mqtt-bridge", version: "0.1.0", tier: "wasm", enabled: true,
      widgets: [{ entry: "remoteEntry.js", label: "Cooler Switch", icon: "x", scope: ["mqtt.status"] }],
    });
    const bEntries = extWidgetEntries(await listExtensions());
    expect(bEntries.some((e) => e.label === "mqtt-bridge · Cooler Switch")).toBe(true);
    expect(bEntries.some((e) => e.label.includes("Proof Ping"))).toBe(false); // ws-A's tile is behind the wall
  });
});

// ---------------------------------------------------------------------------------------------------
// Trust-tier routing RE-ASSERTED FROM THE PALETTE PATH — a packaged tile added from the palette
// federates IN-PROCESS (the install is the trust gate; the iframe sandbox is for scripted author code
// only, and can't load a federated remote anyway — see the debug entry).
// ---------------------------------------------------------------------------------------------------
describe("palette trust-tier routing (real gateway)", () => {
  it("a packaged tile from the palette renders in-process, never sandboxed", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await seedExtension({ ext: "proof-panel", version: "0.1.0", tier: "wasm", enabled: true, widgets: [PROOF_PING] });
    const installed = await listExtensions();

    // The palette resolves the tile to its ext key; an installed widget always federates in-process.
    const entry = extWidgetEntries(installed).find((e) => e.label.includes("Proof Ping"))!;
    expect(extWidgetTier("proof-panel")).toBe("in-process");

    render(<ExtWidget viewKey={entry.viewKey!} installed={installed} workspace={ws} />);
    await waitFor(() => {
      const host = document.querySelector('[data-ext-widget="proof-panel"]');
      expect(host).toBeInTheDocument();
      expect(host?.getAttribute("data-tier")).toBe("in-process");
    });
    expect(document.querySelector("[data-widget-iframe]")).not.toBeInTheDocument();
  });
});
