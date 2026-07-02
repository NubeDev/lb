import { describe, expect, it } from "vitest";
import { computeFolderExposure, resolveChain, type ExposureRecord } from "./exposure";
import vectors from "./exposure-vectors.json";

// The SAME golden vectors the C++ engine implementation runs against — this is
// the cross-language contract (see ../../EXPOSURE_SPEC.md). A change here without
// a matching vector means the two implementations can silently drift.

interface ComputeVector {
  name: string;
  in: Parameters<typeof computeFolderExposure>[0];
  out: ExposureRecord[];
}
interface ResolveVector {
  name: string;
  in: { facets: Record<string, Record<string, { childComponent: number; chain: boolean }>>; folder: number; prop: number };
  out: number | null;
}

describe("computeFolderExposure — golden vectors", () => {
  for (const v of vectors.computeFolderExposure as unknown as ComputeVector[]) {
    it(v.name, () => {
      expect(computeFolderExposure(v.in)).toEqual(v.out);
    });
  }
});

describe("resolveChain — golden vectors", () => {
  for (const v of vectors.resolveChain as unknown as ResolveVector[]) {
    it(v.name, () => {
      const recordFor = (component: number, prop: number) => {
        const r = v.in.facets[String(component)]?.[String(prop)];
        return r ? ({ prop, side: "input", ...r } as ExposureRecord) : undefined;
      };
      expect(resolveChain(recordFor, v.in.folder, v.in.prop) ?? null).toBe(v.out);
    });
  }
});

describe("computeFolderExposure — properties", () => {
  it("is deterministic and sorted by prop", () => {
    const input = {
      folder: 1,
      parents: { "2": 1, "3": 1, "9": 0, "1": 0 },
      edges: [
        { s: 3, sp: 303, t: 9, tp: 909 },
        { s: 9, sp: 901, t: 2, tp: 202 },
      ],
    };
    const out = computeFolderExposure(input);
    expect(out.map((r) => r.prop)).toEqual([202, 303]); // sorted regardless of edge order
  });

  it("survives a cyclic parent map without hanging", () => {
    const out = computeFolderExposure({
      folder: 1,
      parents: { "2": 3, "3": 2 }, // cycle, neither reaches folder 1
      edges: [{ s: 2, sp: 20, t: 9, tp: 90 }],
    });
    expect(out).toEqual([]); // 2 is not inside 1 → no boundary
  });
});
