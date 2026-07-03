// Pure-logic unit tests for the widget builder's data layer (no gateway): the source-picker mapping
// (friendly label → {tool,args}, never a raw name shown), the typed argsTemplate {{value}} fill, and
// the trust-tier routing default (everything iframes unless an allow-listed key). These cover the
// scope's "source picker", "control args", and "trust-tier routing" decisions deterministically; the
// real-gateway round-trips live in *.gateway.test.tsx.

import { describe, expect, it } from "vitest";

import { buildSourceEntries, extensionEntries, extWidgetEntries } from "./sourcePicker";
import { seedEntryId } from "./sourcePicker";
import { fillArgs } from "../views/argsTemplate";
import { extWidgetTier, scriptedTier, isTrustedKey } from "./trust";
import { cellLabel } from "@/lib/dashboard";
import type { Cell } from "@/lib/dashboard";
import type { ExtRow } from "@/lib/ext/ext.api";

function v2cell(over: Partial<Cell>): Cell {
  return {
    i: "c1", x: 0, y: 0, w: 4, h: 3, v: 2,
    widget_type: "chart", binding: { series: "" }, ...over,
  } as Cell;
}

const mqttExt: ExtRow = {
  ext: "mqtt-bridge",
  version: "0.1.0",
  tier: "wasm",
  enabled: true,
  running: true,
  health: "ok",
  restart_count: 0,
  ui: { entry: "remoteEntry.js", label: "MQTT", icon: "x", scope: ["mqtt.status"] },
  widgets: [
    { entry: "remoteEntry.js", label: "Cooler Switch", icon: "x", scope: ["mqtt.status", "mqtt.publish"] },
  ],
};

describe("source picker", () => {
  it("maps a series name to a series.read source (author never types a tool name)", () => {
    const entries = buildSourceEntries(["cooler.temp"], []);
    const series = entries.find((e) => e.group === "series");
    expect(series?.label).toBe("cooler.temp"); // friendly label
    expect(series?.source).toEqual({ tool: "series.read", args: { series: "cooler.temp" } });
  });

  it("offers a live (series.watch) entry per series", () => {
    const entries = buildSourceEntries(["cooler.temp"], []);
    const live = entries.find((e) => e.group === "live");
    expect(live?.source).toEqual({ tool: "series.watch", args: { series: "cooler.temp" } });
  });

  it("splits an extension's tools into READ sources and WRITE actions by name", () => {
    const entries = extensionEntries([mqttExt]);
    const read = entries.find((e) => e.label.includes("mqtt.status"));
    const write = entries.find((e) => e.label.includes("mqtt.publish"));
    // mqtt.status → a read source in the 'extension' group; mqtt.publish → a write action.
    expect(read?.group).toBe("extension");
    expect(read?.writes).toBe(false);
    expect(read?.source).toEqual({ tool: "mqtt.status", args: {} });
    expect(write?.group).toBe("action");
    expect(write?.writes).toBe(true);
    expect(write?.action).toEqual({ tool: "mqtt.publish", argsTemplate: {} });
  });

  it("skips a disabled extension", () => {
    expect(extensionEntries([{ ...mqttExt, enabled: false }])).toHaveLength(0);
  });
});

describe("packaged-tile entries (extWidgetEntries)", () => {
  // A two-tile extension proves "one entry per [[widget]] tile" — the cell key already carries
  // `/<widget>`, so N tiles generalize from proof-panel's one.
  const twoTileExt: ExtRow = {
    ...mqttExt,
    ext: "proof-panel",
    widgets: [
      { entry: "remoteEntry.js", label: "Proof Ping", icon: "shield-check", scope: ["series.latest"] },
      { entry: "remoteEntry.js", label: "Proof Trend", icon: "activity", scope: ["series.find"] },
    ],
  };

  it("emits ONE widget entry per [[widget]] tile, keyed by the renderer's widgetIdOf slug", () => {
    const entries = extWidgetEntries([twoTileExt]);
    expect(entries).toHaveLength(2);
    const ping = entries[0];
    expect(ping.group).toBe("widget");
    expect(ping.label).toBe("proof-panel · Proof Ping"); // `<ext> · <tile.label>`, not a tool name
    expect(ping.icon).toBe("shield-check");
    // The view key MUST match the key ExtWidget parses (slug = lowercase, non-alnum → '-').
    expect(ping.viewKey).toBe("ext:proof-panel/proof-ping");
    expect(ping.writes).toBe(false);
    // A widget entry resolves to a view, not a `{tool,args}` source/action — the tile owns its data.
    expect(ping.source).toBeUndefined();
    expect(ping.action).toBeUndefined();
    expect(entries[1].viewKey).toBe("ext:proof-panel/proof-trend");
  });

  it("contributes no tiles from a disabled extension", () => {
    expect(extWidgetEntries([{ ...twoTileExt, enabled: false }])).toHaveLength(0);
  });

  it("is folded into buildSourceEntries alongside the tool entries (both are offered)", () => {
    const entries = buildSourceEntries([], [twoTileExt]);
    // The same extension contributes BOTH a packaged-tile entry (group 'widget') and its tool entries.
    expect(entries.some((e) => e.group === "widget" && e.label === "proof-panel · Proof Ping")).toBe(true);
    expect(entries.some((e) => e.group === "extension")).toBe(true);
  });
});

describe("Slice 1 — widget settings/config (title + edit-mode seeding)", () => {
  it("cellLabel uses the author title when set", () => {
    expect(cellLabel(v2cell({ title: "Web01 CPU", source: { tool: "series.read" } }))).toBe("Web01 CPU");
  });

  it("cellLabel falls back to the derived label (source tool) when no title", () => {
    expect(cellLabel(v2cell({ source: { tool: "series.read" } }))).toBe("series.read");
  });

  it("cellLabel never returns empty (falls back to view/widget_type)", () => {
    expect(cellLabel(v2cell({ view: "stat" }))).toBe("stat");
  });

  it("seedEntryId resolves a series cell back to its picker entry (for edit-mode seeding)", () => {
    const entries = buildSourceEntries(["cooler.temp", "fryer.state"], []);
    const cell = v2cell({ source: { tool: "series.read", args: { series: "fryer.state" } } });
    const id = seedEntryId(cell, entries);
    expect(entries.find((e) => e.id === id)?.label).toBe("fryer.state");
  });

  it("seedEntryId resolves a packaged ext tile by its view key", () => {
    const ext: ExtRow = { ...mqttExt, ext: "proof-panel", widgets: [
      { entry: "remoteEntry.js", label: "Proof Ping", icon: "x", scope: ["series.latest"] },
    ] };
    const entries = buildSourceEntries([], [ext]);
    const id = seedEntryId(v2cell({ view: "ext:proof-panel/proof-ping" }), entries);
    expect(entries.find((e) => e.id === id)?.viewKey).toBe("ext:proof-panel/proof-ping");
  });

  it("seedEntryId resolves a store.query cell to the SQL source", () => {
    const entries = buildSourceEntries([], []);
    const id = seedEntryId(v2cell({ source: { tool: "store.query", args: { sql: "SELECT 1" } } }), entries);
    expect(entries.find((e) => e.id === id)?.group).toBe("sql");
  });
});

describe("control argsTemplate fill (typed {{value}})", () => {
  it("substitutes only the exact {{value}} leaf, preserving type", () => {
    const filled = fillArgs({ topic: "acme/cooler/defrost", payload: "{{value}}" }, true);
    expect(filled).toEqual({ topic: "acme/cooler/defrost", payload: true }); // bool stays a bool
  });

  it("fills a numeric slider value as a number", () => {
    expect(fillArgs({ level: "{{value}}" }, 42)).toEqual({ level: 42 });
  });

  it("leaves a template with no slot untouched", () => {
    expect(fillArgs({ topic: "x" }, 1)).toEqual({ topic: "x" });
  });
});

describe("trust-tier routing", () => {
  it("renders an INSTALLED extension widget in-process (the install is the trust gate)", () => {
    // An installed extension passed the publish/install capability gate, so its widget federates
    // in-process — the tier its bundle is built for (bare `react` resolves via the shell import map).
    // (Was iframe-by-default; that tier can't load a federated remote — see the debug entry.)
    expect(extWidgetTier("proof-panel")).toBe("in-process");
    expect(extWidgetTier("any-installed-ext")).toBe("in-process");
    expect(extWidgetTier(undefined)).toBe("in-process");
  });

  it("always sandboxes a scripted view (author code typed into a cell never runs in-process)", () => {
    expect(scriptedTier()).toBe("iframe");
  });

  it("keeps the allow-list helper for a future restrict-which-publisher tier (empty by default)", () => {
    expect(isTrustedKey("some-key")).toBe(false);
  });
});
