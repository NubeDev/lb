// Render benchmark — the layer the pure-function benches DON'T cover. Mounts the
// table over N components and measures React mount cost. A regression like
// "every cell becomes a stateful component (useState/useEffect)" — which a pure
// bench can't see — shows up here as a jump in mount time. Pair with the in-app
// diagnostics (renders/s, longTasks), which are the real per-frame perf gate.

import { bench, describe } from "vitest";
import { render, cleanup } from "@testing-library/react";
import { CollectionWidget } from "./CollectionWidget";
import { useStructural, useValues, propertyDataType } from "../lib/store";
import { serializeFacet, FACET_PROP } from "../lib/facet";
import type { Component, Property } from "../lib/engine-types";

const PARENT = 1;
const N = 100;

function prop(uid: number, componentUid: number, category: number, value: unknown, systemRole?: number): Property {
  return { uid, componentUid, category, value, statusFlags: 0, systemRole } as Property;
}

function seed(n: number) {
  const components = new Map<number, Component>();
  const values = new Map<number, unknown>();
  for (let i = 0; i < n; i++) {
    const uid = 100 + i;
    const b = 1000 + i * 10;
    const [in1, in2, out, fac] = [b, b + 1, b + 2, b + 3];
    const facet = serializeFacet(new Map([[out, { label: "Out", unit: "°C", decimals: 1 }]]));
    const properties: Record<string, Property> = {
      in1: prop(in1, uid, 0, 0),
      in2: prop(in2, uid, 0, 0),
      out: prop(out, uid, 1, 0),
      [FACET_PROP]: prop(fac, uid, 0, facet, 2),
    };
    components.set(uid, { uid, name: `c${uid}`, type: "math::add", path: `root/c${uid}`, parent: PARENT, properties });
    propertyDataType.set(in1, 0);
    propertyDataType.set(in2, 0);
    propertyDataType.set(out, 0);
    values.set(in1, i);
    values.set(in2, i * 2);
    values.set(out, i * 3);
  }
  useStructural.setState({ components, linkedProps: new Set() } as never);
  useValues.setState({ values, version: 1 } as never);
}

seed(N);

describe(`CollectionWidget render (${N} components, ~${N * 3} live cells)`, () => {
  bench(
    "mount",
    () => {
      render(<CollectionWidget currentParentUid={PARENT} selectedUids={[]} />);
      cleanup();
    },
    { teardown: cleanup },
  );
});
