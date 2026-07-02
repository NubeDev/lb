import type { Component, Edge, PropertyCategory } from "./engine-types";
import { ROLE_NORMAL } from "./engine-types";

// Candidate logic for the "Connect to…" picker (pure; the picker renders these).

export interface ConnectCandidate {
  propUid: number;
  propName: string;
}
export interface ConnectGroup {
  componentUid: number;
  componentName: string;
  path: string;
  sibling: boolean; // shares the source's parent
  isParent: boolean; // the source's own container (feed-through target)
  isChild: boolean; // nested inside the source component
  props: ConnectCandidate[];
}

// Input prop uids that already have an incoming edge — an input takes a single
// source, so when wiring FROM an output these are hidden.
export function takenInputUids(edges: Iterable<Edge>): Set<number> {
  const taken = new Set<number>();
  for (const e of edges) if (e.targetPropertyUid != null) taken.add(e.targetPropertyUid);
  return taken;
}

// Preference order: parent (feed-through) → same level → children → elsewhere.
export const connectTier = (g: ConnectGroup): number =>
  g.isParent ? 0 : g.sibling ? 1 : g.isChild ? 2 : 3;

// Build the per-component candidate groups for a connect-from source, keeping
// only props of the wanted category/role that aren't already taken, then sort by
// tier (and name, or path within "elsewhere").
export function buildConnectGroups(
  components: Component[],
  opts: {
    sourceComponentUid: number;
    sourceParent: number | undefined;
    wantCategory: PropertyCategory;
    taken: Set<number>;
  },
): ConnectGroup[] {
  const { sourceComponentUid, sourceParent, wantCategory, taken } = opts;
  const groups: ConnectGroup[] = [];
  for (const c of components) {
    if (c.uid === sourceComponentUid) continue;
    const props: ConnectCandidate[] = [];
    for (const [name, p] of Object.entries(c.properties)) {
      if (p.category !== wantCategory) continue;
      if ((p.systemRole ?? ROLE_NORMAL) !== ROLE_NORMAL) continue;
      if (taken.has(p.uid)) continue;
      props.push({ propUid: p.uid, propName: name });
    }
    if (props.length === 0) continue;
    props.sort((a, b) => a.propName.localeCompare(b.propName));
    groups.push({
      componentUid: c.uid,
      componentName: c.name || c.type,
      path: c.path,
      sibling: sourceParent !== undefined && c.parent === sourceParent,
      isParent: sourceParent !== undefined && c.uid === sourceParent,
      isChild: c.parent === sourceComponentUid,
      props,
    });
  }
  groups.sort((a, b) => {
    const ta = connectTier(a);
    const tb = connectTier(b);
    if (ta !== tb) return ta - tb;
    return ta === 3 ? a.path.localeCompare(b.path) : a.componentName.localeCompare(b.componentName);
  });
  return groups;
}

// A path-style filter ("add1/add2/ad") splits at the LAST slash into a folder
// SCOPE the path must contain, and a TERM matched against the component name or
// the path tail BELOW that scope (so it finds matches in that folder AND deeper,
// and the term can't accidentally match folder names in the scope). A plain term
// (no slash) matches name / path-tail / prop name anywhere.
export function filterConnectGroups(groups: ConnectGroup[], filter: string): ConnectGroup[] {
  const f = filter.trim().toLowerCase();
  if (!f) return groups;
  const slash = f.lastIndexOf("/");
  const pathScope = slash >= 0 ? f.slice(0, slash) : "";
  const term = slash >= 0 ? f.slice(slash + 1) : f;
  const out: ConnectGroup[] = [];
  for (const g of groups) {
    const path = g.path.toLowerCase();
    if (pathScope && !path.includes(pathScope)) continue;
    if (!term) {
      out.push(g);
      continue;
    }
    const tail = pathScope ? path.slice(path.indexOf(pathScope) + pathScope.length) : path;
    if (g.componentName.toLowerCase().includes(term) || tail.includes(term)) {
      out.push(g);
      continue;
    }
    const props = g.props.filter((p) => p.propName.toLowerCase().includes(term));
    if (props.length > 0) out.push({ ...g, props });
  }
  return out;
}
