import { describe, expect, it, vi } from "vitest";

import { decodeCov, decodeTopology, dispatchFrame, type CovFrame, type TopologyFrame } from "./frames";
import type { StreamHandlers } from "@nube/ce-wiresheet";

// The STATUS section wire tag (vendored `engine-types.ts` `TYPE_STATUS`) — the literal S6 protocol value
// frames.ts builds, asserted here so a drift in the tag would fail the test.
const TYPE_STATUS = 0x40;

describe("frames.decodeCov", () => {
  it("decodes a cov frame's values into one non-STATUS section keyed by uid", () => {
    const frame: CovFrame = {
      kind: "cov",
      ts: 171234,
      values: [
        { uid: 1000100, v: 4.2 },
        { uid: 1000101, v: 7 },
      ],
    };
    const df = decodeCov(frame);
    expect(df.timestampMs).toBe(171234);
    const valueSection = df.sections.find((s) => s.typeTag !== TYPE_STATUS)!;
    expect(Array.from(valueSection.uids)).toEqual([1000100, 1000101]);
    expect(Array.from(valueSection.values as ArrayLike<unknown>)).toEqual([4.2, 7]);
    // No status → no STATUS section (clean tick).
    expect(df.sections.some((s) => s.typeTag === TYPE_STATUS)).toBe(false);
  });

  it("routes nonzero status flags into a STATUS section", () => {
    const frame: CovFrame = {
      kind: "cov",
      ts: 1,
      values: [{ uid: 5, v: true }],
      status: [{ uid: 5, s: 3 }],
    };
    const df = decodeCov(frame);
    const status = df.sections.find((s) => s.typeTag === TYPE_STATUS)!;
    expect(status).toBeDefined();
    expect(Array.from(status.uids)).toEqual([5]);
    expect(Array.from(status.values as ArrayLike<number>)).toEqual([3]);
  });

  it("coerces a >2^53 integer arriving as a string back to a bigint (lossless)", () => {
    const big = "9007199254740992"; // 2^53 — the first unsafe integer, stringified by the sidecar.
    const frame: CovFrame = {
      kind: "cov",
      ts: 0,
      values: [
        { uid: 1, v: big },
        { uid: 2, v: "-9007199254740992" },
        { uid: 3, v: "plain-string" }, // a genuine string value passes through untouched
      ],
    };
    const df = decodeCov(frame);
    const values = Array.from(df.sections[0].values as ArrayLike<unknown>);
    expect(values[0]).toBe(9007199254740992n);
    expect(values[1]).toBe(-9007199254740992n);
    expect(values[2]).toBe("plain-string");
  });
});

describe("frames.decodeTopology", () => {
  it("maps an added frame to topologyAdded with uid descriptors", () => {
    const frame: TopologyFrame = { kind: "topology", ts: 0, msg: { op: "added", seq: 9, componentUids: [10, 11], edgeUids: [20] } };
    const msg = decodeTopology(frame);
    expect(msg.type).toBe("topologyAdded");
    if (msg.type === "topologyAdded") {
      expect(msg.seq).toBe(9);
      expect(msg.components.map((c) => c.uid)).toEqual([10, 11]);
      expect(msg.edges.map((e) => e.uid)).toEqual([20]);
    }
  });

  it("maps a removed frame to topologyRemoved with the uid lists", () => {
    const frame: TopologyFrame = { kind: "topology", ts: 0, msg: { op: "removed", seq: 3, componentUids: [1], edgeUids: [2] } };
    const msg = decodeTopology(frame);
    expect(msg.type).toBe("topologyRemoved");
    if (msg.type === "topologyRemoved") {
      expect(msg.componentUids).toEqual([1]);
      expect(msg.edgeUids).toEqual([2]);
    }
  });
});

describe("frames.dispatchFrame", () => {
  function handlers(): StreamHandlers & { frames: unknown[]; topos: unknown[] } {
    const frames: unknown[] = [];
    const topos: unknown[] = [];
    return {
      frames,
      topos,
      onSchema: vi.fn(),
      onFrame: (f) => frames.push(f),
      onTopology: (t) => topos.push(t),
      onStatus: vi.fn(),
    };
  }

  it("routes cov → onFrame and topology → onTopology", () => {
    const h = handlers();
    dispatchFrame({ kind: "cov", ts: 1, values: [{ uid: 1, v: 2 }] }, h);
    dispatchFrame({ kind: "topology", ts: 0, msg: { op: "changed", seq: 1, componentUids: [1] } }, h);
    expect(h.frames).toHaveLength(1);
    expect(h.topos).toHaveLength(1);
  });

  it("ignores an unknown frame kind without throwing (forward-compat)", () => {
    const h = handlers();
    expect(() => dispatchFrame({ kind: "future" } as never, h)).not.toThrow();
    expect(h.frames).toHaveLength(0);
    expect(h.topos).toHaveLength(0);
  });
});
