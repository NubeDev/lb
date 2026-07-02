// Drag a PLACED shape to reposition it — the counterpart to use-drag-place.tsx
// (which places NEW shapes). Pointer-down on a shape arms the gesture; once the
// cursor passes a small threshold it becomes a drag: each move projects the cursor
// onto the z=0 plane, snaps (grid + anchor magnetism, the SAME snap() the placement
// path uses — so dragging a fan onto another fan's anchor connects them), and
// writes a transient drag position; pointer-up commits ONE undo step (store.endDrag).
//
// ONE responsibility: the move gesture. Lives inside the r3f tree (uses useThree for
// the camera). While a drag is in flight the store's `drag` is set, which the
// CameraRig reads to freeze MapControls pan so the shape — not the view — follows
// the cursor. A press that never passes the threshold stays a plain click (select).

import { useRef } from "react";
import * as THREE from "three";
import { useThree, type ThreeEvent } from "@react-three/fiber";
import { snap } from "./use-snap";
import { worldAnchors } from "./world-anchors";
import { useSceneStore } from "../state/scene-store";
import { tokens } from "../theme/tokens";

/** pixels the pointer must travel before a press becomes a drag (not a click) */
const DRAG_THRESHOLD_PX = 4;

/** Pointer handlers for a draggable shape. Spread onto the shape's <group>. */
export function useDragMove(id: string) {
  const { camera, gl } = useThree();
  const plane = useRef(new THREE.Plane(new THREE.Vector3(0, 0, 1), 0));
  const raycaster = useRef(new THREE.Raycaster());
  const hit = useRef(new THREE.Vector3());
  // gesture state in a ref — handlers must not depend on re-renders to see it
  const g = useRef<{ startX: number; startY: number; active: boolean } | null>(null);

  function toWorld(clientX: number, clientY: number): { x: number; y: number } | null {
    const rect = gl.domElement.getBoundingClientRect();
    const ndc = new THREE.Vector2(
      ((clientX - rect.left) / rect.width) * 2 - 1,
      -((clientY - rect.top) / rect.height) * 2 + 1,
    );
    raycaster.current.setFromCamera(ndc, camera);
    return raycaster.current.ray.intersectPlane(plane.current, hit.current)
      ? { x: hit.current.x, y: hit.current.y }
      : null;
  }

  function onPointerDown(e: ThreeEvent<PointerEvent>) {
    // placement/chain tool owns clicks — only the select tool drags placed shapes
    const s = useSceneStore.getState();
    if (s.armedType || s.tool !== "select") return;
    if (!s.doc.shapes[id]) return;
    e.stopPropagation();
    g.current = { startX: e.clientX, startY: e.clientY, active: false };
    (e.target as Element).setPointerCapture?.(e.pointerId);
  }

  function onPointerMove(e: ThreeEvent<PointerEvent>) {
    const cur = g.current;
    if (!cur) return;
    if (!cur.active) {
      if (Math.hypot(e.clientX - cur.startX, e.clientY - cur.startY) < DRAG_THRESHOLD_PX) return;
      cur.active = true;
      // select what we're moving (so the halo tracks it) before the first move
      const s = useSceneStore.getState();
      if (!s.selection.includes(id)) s.select([id]);
    }
    e.stopPropagation();
    const w = toWorld(e.clientX, e.clientY);
    if (!w) return;
    const s = useSceneStore.getState();
    // anchor magnetism against every OTHER shape's anchors (exclude self so a shape
    // never snaps to its own anchor); grid snap when enabled — anchor wins
    const anchors = worldAnchors(s.doc).filter((a) => a.shapeId !== id);
    const p = snap(w.x, w.y, { grid: s.snapEnabled ? tokens.grid.step : 0, anchors });
    s.setDrag({ id, x: p.x, y: p.y }); // transient; committed on pointer-up
  }

  function onPointerUp(e: ThreeEvent<PointerEvent>) {
    const cur = g.current;
    g.current = null;
    if (!cur) return;
    (e.target as Element).releasePointerCapture?.(e.pointerId);
    if (!cur.active) return; // a click, not a drag — leave selection to ShapeNode.onClick
    e.stopPropagation();
    useSceneStore.getState().endDrag(); // folds the transient move into one undo step
  }

  return { onPointerDown, onPointerMove, onPointerUp };
}
