// Pure-logic unit tests for the widget builder's data layer (no gateway): the source-picker mapping
// (friendly label → {tool,args}, never a raw name shown), the typed argsTemplate {{value}} fill, and
// the trust-tier routing default (everything iframes unless an allow-listed key). These cover the
// scope's "source picker", "control args", and "trust-tier routing" decisions deterministically; the
// real-gateway round-trips live in *.gateway.test.tsx.

import { describe, expect, it } from "vitest";

import { buildSourceEntries, extensionEntries } from "./sourcePicker";
import { fillArgs } from "../views/argsTemplate";
import { extWidgetTier, scriptedTier, isTrustedKey } from "./trust";
import type { ExtRow } from "@/lib/ext/ext.api";

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
  it("iframes a non-allow-listed extension widget by default (in-process is opt-in)", () => {
    expect(extWidgetTier("some-untrusted-key")).toBe("iframe");
    expect(isTrustedKey("some-untrusted-key")).toBe(false);
  });

  it("always sandboxes a scripted view (author code never in-process)", () => {
    expect(scriptedTier()).toBe("iframe");
  });
});
