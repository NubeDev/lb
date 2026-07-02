import { useEffect, useRef } from "react";
import { useStore as useRfStore, useReactFlow } from "@xyflow/react";
import { NODE_W } from "./FunctionBlock";

// Subscribes the WS value plane to only the components currently in (or near)
// the viewport, instead of every component in the folder. Pairs with
// onlyRenderVisibleElements: we already render only what's on screen, so
// streaming values for the off-screen majority is pure waste — at 182 nodes
// with ~10 visible, this cuts the value traffic (and the store churn behind
// the lag) by ~18×.
//
// Debounced so a continuous pan/zoom doesn't thrash subscribe/unsubscribe (and
// the snapshot frame each new subscription triggers). A generous flow-space
// margin pre-subscribes nodes just off-screen so they're already live by the
// time you pan to them. setDesiredSubscription diffs internally, so a settle
// that doesn't change the visible set sends nothing.

// Flow-space padding added around the viewport on every side.
const MARGIN = 400;
// Fallback node height when RF hasn't measured a node yet (off-screen nodes
// under onlyRenderVisibleElements aren't measured). Overestimate is safe — it
// just subscribes a few extra rows' worth.
const EST_H = 240;
const DEBOUNCE_MS = 200;

export function VisibilitySub({ onVisible }: { onVisible: (uids: Set<number>) => void }) {
  const rf = useReactFlow();
  // Re-evaluate on pan/zoom (transform) and on node-set changes (folder load).
  const transform = useRfStore((s) => s.transform);
  const nodeCount = useRfStore((s) => s.nodes.length);
  const timer = useRef<number | null>(null);

  useEffect(() => {
    if (timer.current != null) window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      const [tx, ty, zoom] = transform;
      if (!zoom) return;
      const W = window.innerWidth;
      const H = window.innerHeight;
      // Screen viewport → flow coords, expanded by MARGIN.
      const vx0 = (0 - tx) / zoom - MARGIN;
      const vy0 = (0 - ty) / zoom - MARGIN;
      const vx1 = (W - tx) / zoom + MARGIN;
      const vy1 = (H - ty) / zoom + MARGIN;

      const visible = new Set<number>();
      for (const n of rf.getNodes()) {
        if (n.type === "ghost") continue;
        const px = n.position.x;
        const py = n.position.y;
        const w = n.width ?? NODE_W;
        const h = n.measured?.height ?? EST_H;
        // AABB intersection with the padded viewport.
        if (px + w >= vx0 && px <= vx1 && py + h >= vy0 && py <= vy1) {
          const uid = Number(n.id);
          if (Number.isFinite(uid)) visible.add(uid);
        }
      }
      onVisible(visible);
    }, DEBOUNCE_MS);
    return () => {
      if (timer.current != null) window.clearTimeout(timer.current);
    };
  }, [transform, nodeCount, rf, onVisible]);

  return null;
}
