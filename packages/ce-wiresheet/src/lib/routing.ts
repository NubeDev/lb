import type { Component, Edge } from "./engine-types";
import { exposedPorts, facetFor, rawFacet } from "./facet";

// Cross-folder edge routing — the pure decisions behind reload()'s edge handling.
// (Ghost geometry / RF node+edge construction stay in the editor; this module
// only decides what connects to what.)

export interface EdgePartition {
  inEdges: Edge[]; // both endpoints are direct-child NODES → drawn via the store path
  crossEdges: Edge[]; // involves a folder PORT and/or one off-canvas end → routed below
}

// Index of exposed ports in the current view: a child-prop uid → the visible folder
// projecting it (the shape `exposedPortIndex().index` returns).
export type PortIndex = Map<number, { parentUid: number }>;

// An edge endpoint resolved against the current view.
type ResolvedEnd =
  | { kind: "node"; uid: number; handle: number } // a direct child; handle = its own prop uid
  | { kind: "port"; uid: number; handle: number } // a visible folder + the port's prop uid (its handle)
  | { kind: "off"; uid: number; propName: string; path: string }; // off-canvas

// Resolve an endpoint: a visible node if its component is a direct child, else a
// visible folder PORT if its prop is one this view projects (even when the owning
// component is a deep grandchild), else off-canvas. The port case is what lets a
// folder-of-folders view route edges that touch NO direct-child component.
function resolveEnd(
  uid: number,
  propUid: number | undefined,
  propName: string,
  path: string | undefined,
  childUids: Set<number>,
  index: PortIndex,
): ResolvedEnd {
  if (childUids.has(uid) && propUid != null) return { kind: "node", uid, handle: propUid };
  if (propUid != null) {
    const f = index.get(propUid);
    if (f) return { kind: "port", uid: f.parentUid, handle: propUid };
  }
  return { kind: "off", uid, propName, path: path ?? "" };
}

// Split a view's edges. node↔node stays an inEdge (the store draws it by name).
// Anything touching a folder PORT (node↔port, port↔port) or with exactly one end
// off-canvas (node↔off, port↔off) is a crossEdge for classifyCrossEdge. Edges with
// BOTH ends off-canvas are dropped.
//
// Internal-to-one-folder edges (e.g. a loopback from a folder's exposed output back
// to a deep input inside the SAME folder) are dropped UPSTREAM by the caller via the
// engine's `class === "internal"` on the `GET /edges?subtree=` response (API_REQUESTS
// §2) — they never reach here, so partition no longer needs a container index.
export function partitionEdges(
  edges: Iterable<Edge>,
  childUids: Set<number>,
  index: PortIndex = new Map(),
): EdgePartition {
  const inEdges: Edge[] = [];
  const crossEdges: Edge[] = [];
  for (const e of edges) {
    const sNode = childUids.has(e.sourceUid);
    const tNode = childUids.has(e.targetUid);
    if (sNode && tNode) {
      inEdges.push(e); // both direct-child nodes (incl. a self-loop)
      continue;
    }
    const sAnchored = sNode || (e.sourcePropertyUid != null && index.has(e.sourcePropertyUid));
    const tAnchored = tNode || (e.targetPropertyUid != null && index.has(e.targetPropertyUid));
    if (sAnchored || tAnchored) crossEdges.push(e);
    // else both ends off-canvas → dropped
  }
  return { inEdges, crossEdges };
}

export interface ExposedPortIndex {
  // child-prop uid → the visible component (folder) projecting it as a port
  index: Map<number, { parentUid: number }>;
  // prop uids to property-subscribe so off-canvas ports stream: the port's own
  // value AND the child's __facets (live label/unit/aliases)
  subProps: Set<number>;
}

// Index the exposed ports of the components visible in this view. (Wiring a new
// edge to a port no longer needs a prop→owner remap here — `POST /edge` derives the
// owner from the prop uid via `componentUid`, API_REQUESTS §1.)
export function exposedPortIndex(children: Component[]): ExposedPortIndex {
  const index = new Map<number, { parentUid: number }>();
  const subProps = new Set<number>();
  for (const child of children) {
    for (const ep of exposedPorts(facetFor(child.uid, rawFacet(child.properties)))) {
      index.set(ep.childUid, { parentUid: child.uid });
      subProps.add(ep.childUid);
      if (ep.facet.facetProp != null) subProps.add(ep.facet.facetProp);
    }
  }
  return { index, subProps };
}

// An anchor a visible edge end attaches to: a component uid + the handle id
// (a prop uid). For a node that's the component + its prop; for a folder port it's
// the FOLDER + the port's (deep child) prop uid — FunctionBlock renders both with
// the prop uid as the React Flow handle id, so they're drawn identically.
export interface EdgeAnchor {
  uid: number;
  handle: number;
}

// How a single cross-folder edge should render.
export type CrossEdgeRoute =
  | {
      kind: "edge"; // BOTH ends visible (node and/or folder port) → a normal edge
      edgeUid: number;
      loopBack: boolean;
      source: EdgeAnchor;
      target: EdgeAnchor;
    }
  | {
      kind: "ghost"; // one end off-canvas → a ghost stub on the visible end
      edgeUid: number;
      loopBack: boolean;
      side: "input" | "output"; // which side of the visible node/folder the ghost sits
      visibleUid: number;
      visibleHandle: number; // the visible end's handle (prop uid, own or a folder port)
      visiblePropName: string; // for a NODE end, to resolve its row; ignored for a port
      visibleIsPort: boolean; // the visible end is a folder PORT → port-row geometry
      externalUid: number;
      externalPropName: string;
      externalPath: string;
    };

// Decide how a cross edge routes. Precondition: at least one endpoint is visible
// (it came from partitionEdges().crossEdges). Both visible (node↔port, port↔port,
// or a node↔node that only partitioned here via a port mismatch) → a normal `edge`
// between the two handles. Exactly one visible → a `ghost` stub anchored on the
// visible end (a node row OR a folder port row).
export function classifyCrossEdge(e: Edge, childUids: Set<number>, index: PortIndex): CrossEdgeRoute {
  const s = resolveEnd(e.sourceUid, e.sourcePropertyUid, e.sourceProperty, e.sourcePath, childUids, index);
  const t = resolveEnd(e.targetUid, e.targetPropertyUid, e.targetProperty, e.targetPath, childUids, index);
  const loopBack = e.loopBack === true;

  if (s.kind !== "off" && t.kind !== "off") {
    return {
      kind: "edge",
      edgeUid: e.uid,
      loopBack,
      source: { uid: s.uid, handle: s.handle },
      target: { uid: t.uid, handle: t.handle },
    };
  }

  // Exactly one end is visible. The ghost (the off-canvas end) sits on the input
  // side when the visible end is the SOURCE (it drives out into the ghost), else
  // the output side. (`side: "input"` → ghost drawn on the right, per the editor.)
  const visIsSource = s.kind !== "off";
  const vis = (visIsSource ? s : t) as Exclude<ResolvedEnd, { kind: "off" }>;
  const off = (visIsSource ? t : s) as Extract<ResolvedEnd, { kind: "off" }>;
  return {
    kind: "ghost",
    edgeUid: e.uid,
    loopBack,
    side: visIsSource ? "input" : "output",
    visibleUid: vis.uid,
    visibleHandle: vis.handle,
    visiblePropName: visIsSource ? e.sourceProperty : e.targetProperty,
    visibleIsPort: vis.kind === "port",
    externalUid: off.uid,
    externalPropName: off.propName,
    externalPath: off.path,
  };
}
