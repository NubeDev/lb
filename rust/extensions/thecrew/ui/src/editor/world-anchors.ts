// Every shape's anchors projected into world space (shape-local → t.sx/sy → t.r →
// t.x/y). Shared by the placement gesture (use-drag-place: snap a NEW shape onto an
// anchor) and the move gesture (use-drag-move: snap a dragged shape onto another's
// anchor). ONE responsibility: the local→world anchor projection.

import type { SceneDoc } from "../scene/scene.types";
import type { WorldAnchor } from "./use-snap";
import { SYMBOLS } from "../canvas/ShapeNode";

export function worldAnchors(doc: SceneDoc): WorldAnchor[] {
  const out: WorldAnchor[] = [];
  for (const [shapeId, shape] of Object.entries(doc.shapes)) {
    const def = SYMBOLS[shape.type];
    if (!def) continue;
    const r = shape.t.r ?? 0;
    const cos = Math.cos(r);
    const sin = Math.sin(r);
    for (const a of def.anchors(shape)) {
      const x = a.x * (shape.t.sx ?? 1);
      const y = a.y * (shape.t.sy ?? 1);
      out.push({
        shapeId,
        name: a.name,
        x: shape.t.x + x * cos - y * sin,
        y: shape.t.y + x * sin + y * cos,
      });
    }
  }
  return out;
}
