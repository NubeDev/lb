import { bench, describe } from "vitest";
import { decodeBinaryFrame } from "./lib/wire";
import { TYPE_F64 } from "./lib/engine-types";
import { facetFor, parseFacet, serializeFacet, FACET_PROP } from "./lib/facet";
import { fmtValueFacet, inferDataType } from "./lib/format";
import { layoutPositions } from "./lib/layout";
import { partitionEdges } from "./lib/routing";
import { buildSearchIndex } from "./lib/search";
import { useValues, propertyDataType } from "./lib/store";
import type { Component, Edge, Property, PropertyCategory } from "./lib/engine-types";

// Performance regression harness. Run with: pnpm --filter @nube/ce-wiresheet bench
// (or `bench:compare` to diff against a saved baseline). These model the steady-
// state per-frame work (decode → apply → format across N visible components) plus
// the reload-time pure derivations. A change that pushes work onto the per-frame
// path — e.g. parsing facets per render — shows up as fewer ops/sec here.

const N = 100; // visible components

// --- synthetic scene: N components, each with a facet (label/unit/decimals/alias)
function prop(uid: number, componentUid: number, category: PropertyCategory): Property {
  return { uid, componentUid, category, value: 0, statusFlags: 0 };
}
const comps: Component[] = [];
const streamUids: number[] = [];
for (let i = 0; i < N; i++) {
  const uid = 100 + i;
  const b = 1000 + i * 10;
  const [in1, in2, out, fac] = [b, b + 1, b + 2, b + 3];
  const facet = serializeFacet(
    new Map([
      [out, { label: "Out", unit: "°C", decimals: 2 }],
      [in1, { aliases: [{ code: 0, label: "off" }, { code: 1, label: "on" }] }],
    ]),
  );
  const properties: Record<string, Property> = {
    in1: prop(in1, uid, 0),
    in2: prop(in2, uid, 0),
    out: prop(out, uid, 1),
    [FACET_PROP]: { ...prop(fac, uid, 0), value: facet, systemRole: 2 },
  };
  comps.push({ uid, name: `c${uid}`, type: "math::add", path: `root/c${uid}`, parent: 0, properties });
  streamUids.push(in1, in2, out);
  propertyDataType.set(in1, 0);
  propertyDataType.set(in2, 0);
  propertyDataType.set(out, 0);
}

// --- a binary value frame carrying every streamable prop (F64) ---
function buildF64Frame(uids: number[]): ArrayBuffer {
  const align8 = (n: number) => (n + 7) & ~7;
  const buf = new ArrayBuffer(16 + 16 + uids.length * 4 + 8 + uids.length * 8 + 16);
  const dv = new DataView(buf);
  dv.setUint8(0, 0x01); // update
  dv.setUint8(8, 1); // 1 section
  const start = 16;
  dv.setUint8(start, TYPE_F64);
  dv.setUint32(start + 4, uids.length, true);
  let p = start + 16;
  uids.forEach((u, i) => dv.setUint32(p + i * 4, u, true));
  p = align8(p + uids.length * 4);
  uids.forEach((_, i) => dv.setFloat64(p + i * 8, i * 1.5, true));
  return buf;
}
const frame = buildF64Frame(streamUids);
const frameValues = streamUids.map((_, i) => i * 1.5);

// --- edges: a chain across the components ---
const edges: Edge[] = [];
for (let i = 1; i < N; i++) {
  edges.push({
    uid: 5000 + i,
    sourceUid: 99 + i,
    sourceProperty: "out",
    sourcePropertyUid: 1000 + (i - 1) * 10 + 2,
    targetUid: 100 + i,
    targetProperty: "in2",
    targetPropertyUid: 1000 + i * 10 + 1,
  });
}
const childUids = new Set(comps.map((c) => c.uid));

describe("per-frame hot path (N=100 visible components)", () => {
  bench("decode binary value frame (300 props)", () => {
    decodeBinaryFrame(frame);
  });

  bench("apply frame to the value store", () => {
    useValues.getState().apply(streamUids, frameValues);
  });

  bench("format all rows (what the visible nodes render per frame)", () => {
    for (const c of comps) {
      const facet = facetFor(c.uid, c.properties[FACET_PROP]?.value as string);
      for (const name of ["in1", "in2", "out"]) {
        const pp = c.properties[name];
        const v = useValues.getState().values.get(pp.uid);
        fmtValueFacet(v, propertyDataType.get(pp.uid) ?? inferDataType(v), facet.get(pp.uid));
      }
    }
  });
});

describe("reload-time pure derivations", () => {
  const facetStr = comps[0].properties[FACET_PROP].value as string;
  bench("parseFacet (one component facet)", () => {
    parseFacet(facetStr);
  });
  bench("layoutPositions (100 components)", () => {
    layoutPositions(comps, 200);
  });
  bench("partitionEdges (100 edges)", () => {
    partitionEdges(edges, childUids);
  });
  bench("buildSearchIndex (100 components)", () => {
    buildSearchIndex([{ uid: 0, name: "root", type: "root", path: "root", parent: -1, properties: {}, children: comps }], 0);
  });
});
