// Model unit tests (no I/O): each group maps to the right {tool,args}/view; the SQL entry is always
// offered; disabled extensions contribute nothing; the widget slug matches the renderer's parse.

import { describe, expect, it } from "vitest";
import {
  buildSourceEntries,
  extensionEntries,
  extWidgetEntries,
  flowsEntries,
  rulesEntries,
  selectionOf,
  widgetIdOf,
  SQL_SOURCE_ID,
} from "./sourcePicker";
import type { ExtRow, Flow, NodeDescriptor } from "./types";

const mqtt: ExtRow = {
  ext: "mqtt-bridge",
  enabled: true,
  ui: { entry: "remoteEntry.js", label: "MQTT", icon: "x", scope: ["mqtt.status"] },
  widgets: [{ entry: "remoteEntry.js", label: "Cooler Switch", icon: "x", scope: ["mqtt.status", "mqtt.publish"] }],
};

describe("buildSourceEntries", () => {
  it("maps a series name to series.read + a live series.watch entry", () => {
    const entries = buildSourceEntries({ series: ["cooler.temp"] });
    expect(entries.find((e) => e.group === "series")?.source).toEqual({ tool: "series.read", args: { series: "cooler.temp" } });
    expect(entries.find((e) => e.group === "live")?.source).toEqual({ tool: "series.watch", args: { series: "cooler.temp" } });
  });

  it("always offers the SQL entry (store.query)", () => {
    const sql = buildSourceEntries({}).find((e) => e.id === SQL_SOURCE_ID);
    expect(sql?.source).toEqual({ tool: "store.query", args: { sql: "" } });
  });
});

describe("extensionEntries", () => {
  it("splits tools into READ sources and WRITE actions by name", () => {
    const entries = extensionEntries([mqtt]);
    const read = entries.find((e) => e.label.includes("mqtt.status"));
    const write = entries.find((e) => e.label.includes("mqtt.publish"));
    expect(read?.group).toBe("extension");
    expect(read?.source).toEqual({ tool: "mqtt.status", args: {} });
    expect(write?.group).toBe("action");
    expect(write?.action).toEqual({ tool: "mqtt.publish", argsTemplate: {} });
  });

  it("skips a disabled extension", () => {
    expect(extensionEntries([{ ...mqtt, enabled: false }])).toHaveLength(0);
  });
});

describe("extWidgetEntries", () => {
  it("emits one packaged-tile entry per [[widget]], keyed by the widgetIdOf slug", () => {
    const [tile] = extWidgetEntries([mqtt]);
    expect(tile.group).toBe("widget");
    expect(tile.label).toBe("mqtt-bridge · Cooler Switch");
    expect(tile.viewKey).toBe(`ext:mqtt-bridge/${widgetIdOf({ label: "Cooler Switch" })}`);
    expect(tile.viewKey).toBe("ext:mqtt-bridge/cooler-switch");
    expect(tile.source).toBeUndefined(); // a tile is a view, not a {tool,args}
  });
});

describe("flowsEntries", () => {
  const flows: Flow[] = [{ id: "f1", name: "F1", nodes: [{ id: "n", type: "t" }] }];
  const descriptors: NodeDescriptor[] = [{ type: "t", inputs: ["cmd"], outputs: ["state"] }];

  it("makes an inject Action for an input port and a node_state Source for an output port", () => {
    const entries = flowsEntries(flows, descriptors);
    const input = entries.find((e) => e.id === "flows:in:f1:n:cmd");
    const output = entries.find((e) => e.id === "flows:out:f1:n:state");
    expect(input?.action).toEqual({ tool: "flows.inject", argsTemplate: { id: "f1", node: "n", port: "cmd", value: "{{value}}" } });
    expect(input?.writes).toBe(true);
    expect(output?.source).toEqual({ tool: "flows.node_state", args: { id: "f1", __flowNode: "n", __flowPort: "state" } });
  });

  it("skips a node whose descriptor is missing (honest empty, no guess)", () => {
    expect(flowsEntries([{ id: "f1", name: "F1", nodes: [{ id: "n", type: "UNKNOWN" }] }], descriptors)).toHaveLength(0);
  });
});

describe("rulesEntries", () => {
  it("maps a saved rule to a rules.run read source keyed by rule_id", () => {
    const [rule] = rulesEntries([{ id: "r1", name: "Hourly mean" }]);
    expect(rule.group).toBe("rules");
    expect(rule.id).toBe("rule:r1");
    expect(rule.label).toBe("Hourly mean");
    expect(rule.writes).toBe(false);
    // route:false makes a panel run read-only (no alert fan-out on repaint — slice 2).
    expect(rule.source).toEqual({ tool: "rules.run", args: { rule_id: "r1", route: false } });
  });

  it("falls back to the id when a rule has no name", () => {
    expect(rulesEntries([{ id: "r2", name: "" }])[0].label).toBe("r2");
  });

  it("carries the rule's declared params onto the entry (for a host params form)", () => {
    const [rule] = rulesEntries([
      { id: "r1", name: "By site", params: [{ name: "site", label: "Site" }, { name: "hours" }] },
    ]);
    expect(rule.params).toEqual([{ name: "site", label: "Site" }, { name: "hours" }]);
  });

  it("defaults params to [] for a rule that declares none", () => {
    expect(rulesEntries([{ id: "r1", name: "R" }])[0].params).toEqual([]);
  });

  it("carries a typed param (kind/required/options) through onto the entry", () => {
    const [rule] = rulesEntries([
      { id: "r1", name: "R", params: [{ name: "region", kind: "enum", required: true, options: ["a", "b"] }] },
    ]);
    expect(rule.params).toEqual([
      { name: "region", kind: "enum", required: true, options: ["a", "b"] },
    ]);
  });

  it("is folded into buildSourceEntries under the rules group", () => {
    const entries = buildSourceEntries({ rules: [{ id: "r1", name: "R1" }] });
    expect(entries.find((e) => e.group === "rules")?.source).toEqual({
      tool: "rules.run",
      args: { rule_id: "r1", route: false },
    });
  });
});

describe("selectionOf", () => {
  it("keeps only what a host stores (drops labelling fields)", () => {
    const [series] = buildSourceEntries({ series: ["s"] });
    expect(selectionOf(series)).toEqual({ id: "series:s", source: { tool: "series.read", args: { series: "s" } }, action: undefined, viewKey: undefined });
  });
});
