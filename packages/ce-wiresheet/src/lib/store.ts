import { create } from "zustand";
import type {
  Component,
  Edge,
  PropertyDataType,
  SchemaPropertyEntry,
} from "./engine-types";
import type { DecodedValue } from "./wire";

// Three parallel views of the world:
//   - REST-derived structural data: components map (by UID + by path) and edges.
//     Authoritative for names, paths, kinds, hierarchy, and property metadata
//     (category, systemRole, system, statusFlags-at-fetch-time, componentUid).
//   - WS-derived decode hints: per-property dataType (so binary frame sections
//     can be routed to typed cells) and a live statusFlags map (seeded by the
//     schema bootstrap, updated by the STATUS section of each binary frame).
//   - Live value map keyed by property UID. Mutated in place; a `version` tick
//     drives selector re-evaluation in subscribers.

// --- REST: structural state ----------------------------------------------------

interface StructuralState {
  components: Map<number, Component>;      // by UID
  componentsByPath: Map<string, Component>;
  edges: Map<number, Edge>;                // by UID
  // Prop uids that are an endpoint of ANY edge in the current view's scope —
  // including cross-folder edges (which aren't kept in `edges`, since only
  // both-ends-visible edges are drawn). Used to flag "linked" props in the table.
  linkedProps: Set<number>;
  setNodes(comps: Component[], edges: Edge[]): void;
  setLinkedProps(props: Set<number>): void;
  upsertComponent(c: Component): void;
  removeComponent(uid: number): void;
  upsertEdge(e: Edge): void;
  removeEdge(uid: number): void;
}

// Built incrementally from REST `Component.properties` (each Property carries
// `componentUid`). Kept module-level so the binary frame decoder can fan a uid
// out to a component in O(1) without going through Zustand selectors.
export const propertyToComponent = new Map<number, number>();

function indexComponentProperties(c: Component) {
  for (const p of Object.values(c.properties)) {
    propertyToComponent.set(p.uid, c.uid);
  }
}

function unindexComponentProperties(c: Component) {
  for (const p of Object.values(c.properties)) {
    propertyToComponent.delete(p.uid);
  }
}

export const useStructural = create<StructuralState>((set) => ({
  components: new Map(),
  componentsByPath: new Map(),
  edges: new Map(),
  linkedProps: new Set(),
  setLinkedProps: (linkedProps) => set({ linkedProps }),
  setNodes: (comps, edges) => {
    const cByUid = new Map<number, Component>();
    const cByPath = new Map<string, Component>();
    propertyToComponent.clear();
    const walk = (c: Component) => {
      cByUid.set(c.uid, c);
      cByPath.set(c.path, c);
      indexComponentProperties(c);
      c.children?.forEach(walk);
    };
    comps.forEach(walk);
    const eByUid = new Map<number, Edge>();
    edges.forEach((e) => eByUid.set(e.uid, e));
    set({ components: cByUid, componentsByPath: cByPath, edges: eByUid });
  },
  upsertComponent: (c) =>
    set((s) => {
      const components = new Map(s.components);
      const componentsByPath = new Map(s.componentsByPath);
      const prev = components.get(c.uid);
      if (prev) unindexComponentProperties(prev);
      components.set(c.uid, c);
      componentsByPath.set(c.path, c);
      indexComponentProperties(c);
      return { components, componentsByPath };
    }),
  removeComponent: (uid) =>
    set((s) => {
      const components = new Map(s.components);
      const old = components.get(uid);
      if (old) unindexComponentProperties(old);
      components.delete(uid);
      const componentsByPath = new Map(s.componentsByPath);
      if (old) componentsByPath.delete(old.path);
      // Drop any edges referencing the removed component too.
      const edges = new Map(s.edges);
      for (const [eid, e] of edges) {
        if (e.sourceUid === uid || e.targetUid === uid) edges.delete(eid);
      }
      return { components, componentsByPath, edges };
    }),
  upsertEdge: (e) =>
    set((s) => {
      const edges = new Map(s.edges);
      edges.set(e.uid, e);
      return { edges };
    }),
  removeEdge: (uid) =>
    set((s) => {
      const edges = new Map(s.edges);
      edges.delete(uid);
      return { edges };
    }),
}));

// --- WS: decode hints (dataType + status flags) -------------------------------

// dataType per streamable property uid. Comes from the WS schema bootstrap and
// is overwritten on each (re)configure. Read by the binary frame decoder and
// the value-formatting UI.
export const propertyDataType = new Map<number, PropertyDataType>();

// Live uint32 status bitmask per property uid. Seeded from the WS schema and
// updated by the STATUS section of each binary frame. Mutated in place; the
// `version` tick on the store below drives re-renders.
interface StatusFlagsState {
  flags: Map<number, number>;
  version: number;
  // Apply a batch of (uid, statusFlags) updates from the STATUS section.
  applyStatus(uids: ArrayLike<number>, flags: ArrayLike<number>): void;
  reset(): void;
}

// Notification coalescing. WS frames can arrive far faster than the display
// can paint (we've measured ~100 Hz from the engine). Bumping `version`
// synchronously on every frame makes every store subscriber re-run its
// selector that many times per second — the dominant main-thread cost at
// scale, even when nothing actually re-renders. Instead we mutate the maps in
// place immediately (so a render that happens for any other reason sees fresh
// data) but defer the `version` bump — the signal that triggers selector
// re-evaluation — to one rAF. Many frames between paints collapse into a
// single notification, capping reconcile work at the display rate.
function makeRafBump(set: (fn: (s: { version: number }) => { version: number }) => void) {
  let scheduled = false;
  return () => {
    if (scheduled) return;
    scheduled = true;
    requestAnimationFrame(() => {
      scheduled = false;
      set((s) => ({ version: s.version + 1 }));
    });
  };
}

export const useStatusFlags = create<StatusFlagsState>((set, get) => {
  const bump = makeRafBump(set as never);
  return {
    flags: new Map(),
    version: 0,
    applyStatus: (uids, flags) => {
      const m = get().flags;
      for (let i = 0; i < uids.length; i++) m.set(uids[i], flags[i] as number);
      bump();
    },
    reset: () => set({ flags: new Map(), version: 0 }),
  };
});

// Schema arrival tick. Components subscribe to it so re-renders happen when the
// dataType table (and seeded status flags) fill in.
interface SchemaState {
  version: number;
  bump(): void;
}
export const useSchemaVersion = create<SchemaState>((set) => ({
  version: 0,
  bump: () => set((s) => ({ version: s.version + 1 })),
}));

export function loadSchemaIndices(properties: SchemaPropertyEntry[]) {
  propertyDataType.clear();
  const seeded = new Map<number, number>();
  for (const p of properties) {
    propertyDataType.set(p.uid, p.dataType);
    seeded.set(p.uid, p.statusFlags >>> 0);
  }
  // Replace, don't merge — the schema is the authoritative starting point.
  useStatusFlags.setState({ flags: seeded, version: useStatusFlags.getState().version + 1 });
  useSchemaVersion.getState().bump();
}

// --- WS: live values -----------------------------------------------------------

interface ValuesState {
  values: Map<number, DecodedValue>;
  version: number;
  apply(uids: ArrayLike<number>, values: ArrayLike<DecodedValue> | DecodedValue[]): void;
  reset(): void;
}

export const useValues = create<ValuesState>((set, get) => {
  const bump = makeRafBump(set as never);
  return {
    values: new Map(),
    version: 0,
    // Mutate in place immediately; coalesce the version bump to one rAF (see
    // makeRafBump). Components select specific UIDs from the Map and observe
    // changes on version bump — now at most once per frame regardless of how
    // many WS frames landed since the last paint.
    apply: (uids, vs) => {
      const m = get().values;
      for (let i = 0; i < uids.length; i++) {
        m.set(uids[i], vs[i] as DecodedValue);
      }
      bump();
    },
    reset: () => set({ values: new Map(), version: 0 }),
  };
});
