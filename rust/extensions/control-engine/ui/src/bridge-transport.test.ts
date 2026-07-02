import { describe, expect, it } from "vitest";

import { BridgeTransport, UnmappedPathError } from "./bridge-transport";
import { stubBridge, watchBridge } from "./test/bridge.stub";
import type { EngineRequest } from "@nube/ce-wiresheet";

const APPLIANCE = "ce-studio";

/** A bridge that resolves every control-engine.* verb to a shaped ack, recording each `{tool,args}`. */
function recordingBridge() {
  return stubBridge({
    "control-engine.tree": () => ({ nodes: [], edges: [] }),
    "control-engine.schema": () => ({ manifests: ["m"] }),
    "control-engine.add-node": () => ({ uid: 7, kind: "component" }),
    "control-engine.patch": () => ({ component: { uid: 3 } }),
    "control-engine.set-override": () => ({ ok: true }),
    "control-engine.clear-override": () => ({ ok: true }),
    "control-engine.add-edge": () => ({ uid: 9, kind: "edge" }),
    "control-engine.remove-node": () => ({ deleted: { component_uids: [3], edge_uids: [] } }),
    "control-engine.call-action": () => ({ returns: {} }),
  });
}

// The set of REST paths the vendored `rest.ts` typed wrappers can emit through the seam (derived by
// reading its `http(method, path, body)` call sites). Each entry: the request + the tool it must map to.
const MAPPED: Array<{ req: EngineRequest; tool: string }> = [
  { req: { method: "GET", path: "/schema" }, tool: "control-engine.schema" },
  { req: { method: "GET", path: "/nodes" }, tool: "control-engine.tree" },
  { req: { method: "GET", path: "/nodes?depth=2&withEdges=true" }, tool: "control-engine.tree" },
  { req: { method: "GET", path: "/nodes/uid/42" }, tool: "control-engine.tree" },
  { req: { method: "POST", path: "/nodes", body: { type: "x::y" } }, tool: "control-engine.add-node" },
  { req: { method: "PATCH", path: "/nodes/uid/3", body: { properties: {} } }, tool: "control-engine.patch" },
  {
    req: { method: "PATCH", path: "/overrides/nodes/uid/3", body: { setOverrides: [{ property: "in", value: 1 }] } },
    tool: "control-engine.set-override",
  },
  {
    req: { method: "PATCH", path: "/overrides/nodes/uid/3", body: { clearOverrides: ["in"] } },
    tool: "control-engine.clear-override",
  },
  { req: { method: "POST", path: "/edge", body: { sourceComponent: 1, sourceProperty: 10, targetComponent: 2, targetProperty: 20 } }, tool: "control-engine.add-edge" },
  { req: { method: "DELETE", path: "/nodes/uid/3" }, tool: "control-engine.remove-node" },
  { req: { method: "POST", path: "/call/nodes/uid/3", body: { action: "run" } }, tool: "control-engine.call-action" },
];

describe("BridgeTransport request-map", () => {
  it.each(MAPPED)("maps $req.method $req.path → $tool", async ({ req, tool }) => {
    const bridge = recordingBridge();
    const t = new BridgeTransport(bridge, APPLIANCE);
    await t.request(req);
    const call = bridge.calls.at(-1)!;
    expect(call.tool).toBe(tool);
    // The selected appliance is ALWAYS injected (workspace-scoped routing).
    expect(call.args?.appliance).toBe(APPLIANCE);
  });

  it("throws a LOUD path-naming error for an unmapped path (never a silent 404)", async () => {
    const t = new BridgeTransport(recordingBridge(), APPLIANCE);
    for (const path of ["/undo", "/redo", "/changelog", "/group", "/copy/nodes", "/bulknodes", "/edges"]) {
      await expect(t.request({ method: "POST", path })).rejects.toBeInstanceOf(UnmappedPathError);
      await expect(t.request({ method: "POST", path })).rejects.toThrow(path);
    }
  });
});

describe("BridgeTransport arg translation", () => {
  it("PATCH /nodes/uid/{uid} → patch { node:{uid,kind}, values } (flattening {value} wrappers)", async () => {
    const bridge = recordingBridge();
    const t = new BridgeTransport(bridge, APPLIANCE);
    const out = await t.request({
      method: "PATCH",
      path: "/nodes/uid/3",
      body: { properties: { in: { value: 42 }, mode: { value: "auto" } } },
    });
    const call = bridge.calls.at(-1)!;
    expect(call.tool).toBe("control-engine.patch");
    expect(call.args?.node).toEqual({ uid: 3, kind: "component" });
    expect(call.args?.values).toEqual({ in: 42, mode: "auto" });
    // Result is unwrapped to the component DTO (rest.ts expects the plain `data`).
    expect(out).toEqual({ uid: 3 });
  });

  it("DELETE /nodes/uid/{uid} → remove-node with the keyed node", async () => {
    const bridge = recordingBridge();
    await new BridgeTransport(bridge, APPLIANCE).request({ method: "DELETE", path: "/nodes/uid/8" });
    const call = bridge.calls.at(-1)!;
    expect(call.tool).toBe("control-engine.remove-node");
    expect(call.args?.node).toEqual({ uid: 8, kind: "component" });
  });

  it("POST /nodes → add-node { type, parent, initial_values }", async () => {
    const bridge = recordingBridge();
    await new BridgeTransport(bridge, APPLIANCE).request({
      method: "POST",
      path: "/nodes",
      body: { type: "vendor::block", parentUid: 1, name: "b1", initialValues: { x: { value: 5 } } },
    });
    const call = bridge.calls.at(-1)!;
    expect(call.args?.type).toBe("vendor::block");
    expect(call.args?.parent).toEqual({ uid: 1, kind: "component" });
    expect(call.args?.name).toBe("b1");
    expect(call.args?.initial_values).toEqual({ x: 5 });
  });

  it("PATCH /overrides set → set-override { property, value, ttl_secs }", async () => {
    const bridge = recordingBridge();
    await new BridgeTransport(bridge, APPLIANCE).request({
      method: "PATCH",
      path: "/overrides/nodes/uid/4",
      body: { setOverrides: [{ property: "sp", value: 21, duration: 60 }] },
    });
    const call = bridge.calls.at(-1)!;
    expect(call.tool).toBe("control-engine.set-override");
    expect(call.args).toMatchObject({ node: { uid: 4 }, property: "sp", value: 21, ttl_secs: 60 });
  });
});

describe("BridgeTransport openStream", () => {
  it("arms control-engine.watch and pipes a seeded cov frame into onFrame", async () => {
    const { bridge, emit, unsubscribed } = watchBridge({
      "control-engine.watch": () => ({ series: "ce.ce-studio", subject: "ws/w/series/ce.ce-studio" }),
    });
    const t = new BridgeTransport(bridge, APPLIANCE);
    const frames: unknown[] = [];
    const statuses: string[] = [];
    const stream = t.openStream({
      onSchema() {},
      onTopology() {},
      onFrame: (f) => frames.push(f),
      onStatus: (s) => statuses.push(s),
    });
    // Let the watch-arm promise resolve.
    await Promise.resolve();
    await new Promise((r) => setTimeout(r, 0));

    expect(statuses).toContain("open");
    // The shell delivers `{ payload, seq }`; the payload is the S6 frame.
    emit({ payload: { kind: "cov", ts: 1, values: [{ uid: 100, v: 3.14 }] }, seq: 1 });
    expect(frames).toHaveLength(1);

    stream.close();
    expect(unsubscribed()).toBe(true);
    expect(statuses.at(-1)).toBe("closed");
  });

  it("degrades to onStatus(closed) with NO throw when the bridge has no watch", async () => {
    const bridge = stubBridge({ "control-engine.watch": () => ({ series: "s", subject: "x" }) });
    const t = new BridgeTransport(bridge, APPLIANCE);
    const statuses: string[] = [];
    expect(() =>
      t.openStream({ onSchema() {}, onTopology() {}, onFrame() {}, onStatus: (s) => statuses.push(s) }),
    ).not.toThrow();
    expect(statuses).toContain("closed");
  });
});
