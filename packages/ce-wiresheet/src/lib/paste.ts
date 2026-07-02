import { ROLE_NORMAL, type Component } from "./engine-types";
import { rawFacet, remapFacetUids, FACET_PROP } from "./facet";

// Visual node geometry, mirrored from FunctionBlock so the paste centroid matches
// the RENDERED bounding box (keeps lib/ free of the React component). Width is
// fixed; height = title + one row per visible (ROLE_NORMAL) property.
const NODE_W = 220;
const nodeHeight = (c: Component): number => {
  const rows = Object.values(c.properties ?? {}).filter(
    (p) => (p.systemRole ?? ROLE_NORMAL) === ROLE_NORMAL,
  ).length;
  return 40 /* title */ + rows * 18 /* rows */ + 4;
};

export interface PasteUpdate {
  uid: number;
  position?: { x: number; y: number };
  properties?: Record<string, { value: string }>;
}
export interface PastePlan {
  updates: PasteUpdate[]; // one bulkUpdate payload (positions + facet remap)
  newUids: number[]; // the TOP-LEVEL clones to select after paste
}

// How far (flow units) the pasted cluster's top-left may sit from the paste cursor
// when honouring the copy-time grab offset — keeps a copy made with the pointer far
// from the selection ("within reason") from landing way off near the paste cursor.
const PASTE_CLAMP = 400;
const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));

// Plan a paste from /copy/nodes output: flatten the (possibly nested) cloned
// subtree, translate the TOP-LEVEL clones into place, and remap uid references in
// any copied __facets (the engine copies the facet value verbatim, so it still
// points at the original uids — see API_REQUESTS §0a). Only top-level clones
// (placed directly under the dest) are repositioned and selected; descendants are
// off-canvas inside a pasted folder.
//
// Placement: when `opts.copyCursor` is given, each node keeps its offset from where
// the mouse was at copy time (shift by pasteCursor - copyCursor, clamped within
// reason). Otherwise the visual bounding box is centred on the cursor.
export function planPaste(
  clones: Component[],
  destParentUid: number,
  pasteCursor: { x: number; y: number },
  opts?: {
    copyCursor?: { x: number; y: number };
    uidMap?: { components?: Record<string, number>; properties?: Record<string, number> };
  },
): PastePlan {
  const all: Component[] = [];
  const flatten = (c: Component) => {
    all.push(c);
    c.children?.forEach(flatten);
  };
  clones.forEach(flatten);

  const uidMap = opts?.uidMap;
  const topLevel = all.filter((c) => c.parent === destParentUid);
  const px = (c: Component) => c.metadata?.position?.x ?? 0;
  const py = (c: Component) => c.metadata?.position?.y ?? 0;

  let dx = 0;
  let dy = 0;
  if (topLevel.length) {
    const minX = Math.min(...topLevel.map(px));
    const minY = Math.min(...topLevel.map(py));
    if (opts?.copyCursor) {
      // Preserve the grab point: where the cluster sat relative to the copy mouse,
      // reproduced from the paste mouse. Clamp that offset so a far-away copy mouse
      // still drops the paste near the cursor.
      const offX = clamp(minX - opts.copyCursor.x, -PASTE_CLAMP, PASTE_CLAMP);
      const offY = clamp(minY - opts.copyCursor.y, -PASTE_CLAMP, PASTE_CLAMP);
      dx = pasteCursor.x + offX - minX;
      dy = pasteCursor.y + offY - minY;
    } else {
      // No copy cursor (e.g. legacy clipboard): centre the visual bbox on the cursor.
      const maxX = Math.max(...topLevel.map((c) => px(c) + NODE_W));
      const maxY = Math.max(...topLevel.map((c) => py(c) + nodeHeight(c)));
      dx = pasteCursor.x - (minX + maxX) / 2;
      dy = pasteCursor.y - (minY + maxY) / 2;
    }
  }

  const compMap = uidMap?.components ?? {};
  const propMap = uidMap?.properties ?? {};
  const topSet = new Set(topLevel.map((c) => c.uid));
  const updates: PasteUpdate[] = [];
  for (const c of all) {
    const entry: PasteUpdate = { uid: c.uid };
    if (topSet.has(c.uid)) {
      entry.position = {
        x: Math.round((c.metadata?.position?.x ?? 0) + dx),
        y: Math.round((c.metadata?.position?.y ?? 0) + dy),
      };
    }
    if (uidMap) {
      const raw = rawFacet(c.properties);
      if (raw) {
        const remapped = remapFacetUids(raw, compMap, propMap);
        if (remapped !== raw) entry.properties = { [FACET_PROP]: { value: remapped } };
      }
    }
    if (entry.position || entry.properties) updates.push(entry);
  }
  return { updates, newUids: topLevel.map((c) => c.uid) };
}
