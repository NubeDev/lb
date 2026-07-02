// bridge-source unit tests: the multiplexer (one upstream per series, dedupe, fan-out), backfill
// via series.latest, live via watch, poll fallback, and the deny → null (no-access) path.

import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { stubBridge, rejectingBridge, watchBridge } from "./bridge.stub";
import { createBridgeSource, collectChannels } from "./bridge-source";
import type { SceneDoc } from "../scene/scene.types";

function latest(payload: unknown) {
  return { sample: { payload } };
}

describe("collectChannels", () => {
  it("collects + dedupes every bound channel across shapes", () => {
    const doc: SceneDoc = {
      v: 1,
      camera: "ortho-top",
      shapes: {
        a: { type: "hvac.fan", t: { x: 0, y: 0 }, props: {}, bind: { speed: { channel: "x" }, run: { channel: "y" } } },
        b: { type: "hvac.fan", t: { x: 0, y: 0 }, props: {}, bind: { speed: { channel: "x" } } },
        c: { type: "shape.text", t: { x: 0, y: 0 }, props: {} },
      },
    };
    expect(collectChannels(doc).sort()).toEqual(["x", "y"]);
  });
});

describe("createBridgeSource (page bridge, poll fallback)", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("backfills each subscriber with series.latest and fires immediately", async () => {
    const bridge = stubBridge({ "series.latest": (args) => latest(args?.series === "x" ? 42 : null) });
    const src = createBridgeSource(bridge, ["x"]);
    const seen: unknown[] = [];
    src.subscribe("x", (v) => seen.push(v));
    expect(seen[0]).toBe(null); // immediate fire with the current (not-yet-backfilled) value
    await vi.runOnlyPendingTimersAsync();
    expect(seen).toContain(42); // backfill lands
  });

  it("opens ONE upstream per series regardless of subscriber count (fan-out)", async () => {
    // Use the watch bridge so there is no poll timer to muddy the count: N subscribers must yield
    // exactly ONE backfill call + ONE watch subscription for the series (fan-out, not per-shape).
    const call = vi.fn((args?: Record<string, unknown>) => latest(args?.series === "x" ? 7 : null));
    const { bridge } = watchBridge({ "series.latest": call });
    const src = createBridgeSource(bridge, ["x"]);
    src.subscribe("x", () => {});
    src.subscribe("x", () => {});
    src.subscribe("x", () => {});
    await Promise.resolve();
    expect(call).toHaveBeenCalledTimes(1); // one backfill for three subscribers
    expect(bridge.watch).toHaveBeenCalledTimes(1); // one live upstream for three subscribers
  });

  it("a denied series resolves to null (no-access state), never throws", async () => {
    const src = createBridgeSource(rejectingBridge("out_of_scope: series.latest"), ["x"]);
    const seen: unknown[] = [];
    src.subscribe("x", (v) => seen.push(v));
    await vi.runOnlyPendingTimersAsync();
    expect(seen.every((v) => v === null)).toBe(true);
  });
});

describe("createBridgeSource (widget bridge, live watch)", () => {
  it("backfills then streams live samples to all subscribers", async () => {
    const { bridge, emit } = watchBridge({ "series.latest": () => latest(1) });
    const src = createBridgeSource(bridge, ["x"]);
    const seen: unknown[] = [];
    src.subscribe("x", (v) => seen.push(v));
    await Promise.resolve();
    await Promise.resolve();
    emit("x", { payload: 99 });
    expect(seen).toContain(99);
  });

  it("closes the upstream when the last subscriber leaves (stateless eviction)", async () => {
    const { bridge, unsubscribed } = watchBridge({ "series.latest": () => latest(1) });
    const src = createBridgeSource(bridge, ["x"]);
    const off = src.subscribe("x", () => {});
    off();
    expect(unsubscribed()).toBe(true);
  });

  it("channels() reports the deduped bound set for the picker", () => {
    const { bridge } = watchBridge({});
    expect(createBridgeSource(bridge, ["x", "x", "y"]).channels().sort()).toEqual(["x", "y"]);
  });
});
