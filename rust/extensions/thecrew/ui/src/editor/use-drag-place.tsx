// Palette → canvas placement: translucent ghost of the REAL symbol follows the
// cursor already snapped; drop places defaults; dropping onto an anchor places AND
// connects in one gesture. Also the polyline chain tool for ducts/walls
// (click…click, double-click ends, Esc cancels). ONE responsibility: placement gestures.
//
// INTEGRATION: <DropPlane /> must be rendered as a child of SceneCanvas's <Canvas>
// (inside the r3f tree — it uses useThree for the camera and gl.domElement for
// HTML5 drop). SceneCanvas.tsx is owned by another agent; add `<DropPlane />` there.
//
// "Connect" in v1 = geometric adjacency: snapping onto an anchor places the shape at
// the anchor point, which the playground treats as connected (builder-ux-scope §place).
// There is no edge model yet — the framework adds one; here adjacency IS the edge.
//
// Keyboard split: chain-tool Enter/Esc are handled HERE; Esc/Del/arrows/⌘D in
// use-selection.ts; undo keys in use-undo.ts; V/D/G/Tab/? in Toolbar.tsx.

import { useEffect, useMemo, useRef, useState } from "react";
import * as THREE from "three";
import { useThree, type ThreeEvent } from "@react-three/fiber";
import { Line } from "@react-three/drei";
import { snap, type SnapResult } from "./use-snap";
import { worldAnchors } from "./world-anchors";
import { SYMBOLS } from "../canvas/ShapeNode";
import { defaultShape } from "../scene/defaults";
import { useSceneStore } from "../state/scene-store";
import { ghostMaterial } from "../theme/materials";
import { tokens } from "../theme/tokens";

const CHAIN_TYPES = new Set(["hvac.duct", "plan.wall"]);
const MIME = "thecrew/type";

/** grid + anchor-magnetism snap against the current doc (anchor wins — use-snap.ts) */
function snapPoint(x: number, y: number): SnapResult {
  const { snapEnabled, doc } = useSceneStore.getState();
  return snap(x, y, { grid: snapEnabled ? tokens.grid.step : 0, anchors: worldAnchors(doc) });
}

/** Invisible z=0 plane inside the <Canvas>: click-to-place, ghost preview,
 * HTML5 drop, and the chain (polyline) tool for ducts/walls. */
export function DropPlane() {
  const { camera, gl } = useThree();
  const armedType = useSceneStore((s) => s.armedType);
  const tool = useSceneStore((s) => s.tool);
  const [ghost, setGhost] = useState<SnapResult | null>(null);
  const [chain, setChain] = useState<[number, number][]>([]);
  const chainRef = useRef(chain);
  chainRef.current = chain;

  const chaining = tool === "chain" && !!armedType && CHAIN_TYPES.has(armedType);
  const def = armedType ? SYMBOLS[armedType] : undefined;
  const ghostShape = useMemo(() => (armedType ? defaultShape(armedType) : null), [armedType]);

  // disarming / leaving the chain tool clears any preview state
  useEffect(() => {
    if (!armedType) setGhost(null);
    if (!armedType || tool !== "chain") setChain([]);
  }, [armedType, tool]);

  function finishChain(points: [number, number][]) {
    const s = useSceneStore.getState();
    if (points.length >= 2 && s.armedType) {
      const [fx, fy] = points[0]!;
      // points are stored shape-local: subtract the first point
      const rel = points.map(([x, y]) => [x - fx, y - fy] as [number, number]);
      const base = defaultShape(s.armedType);
      s.placeShape(s.armedType, fx, fy, { props: { ...base.props, points: rel } });
    }
    setChain([]);
    s.armType(null); // also resets tool to "select" — no mode traps
  }

  // chain keyboard: Enter ends the run, Esc cancels it (Toolbar/use-selection do the rest)
  useEffect(() => {
    if (!chaining) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Enter") finishChain(chainRef.current);
      else if (e.key === "Escape") setChain([]);
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [chaining]);

  // HTML5 drop from the palette: pointermove doesn't fire during a native drag, so
  // dragover raycasts client coords onto the z=0 plane manually.
  useEffect(() => {
    const el = gl.domElement.parentElement ?? gl.domElement;
    const raycaster = new THREE.Raycaster();
    const plane = new THREE.Plane(new THREE.Vector3(0, 0, 1), 0);
    const hit = new THREE.Vector3();

    function toWorld(clientX: number, clientY: number): { x: number; y: number } | null {
      const rect = gl.domElement.getBoundingClientRect();
      const ndc = new THREE.Vector2(
        ((clientX - rect.left) / rect.width) * 2 - 1,
        -((clientY - rect.top) / rect.height) * 2 + 1,
      );
      raycaster.setFromCamera(ndc, camera);
      return raycaster.ray.intersectPlane(plane, hit) ? { x: hit.x, y: hit.y } : null;
    }

    function onDragOver(e: DragEvent) {
      if (!e.dataTransfer?.types.includes(MIME)) return;
      e.preventDefault();
      e.dataTransfer.dropEffect = "copy";
      const w = toWorld(e.clientX, e.clientY);
      if (w) setGhost(snapPoint(w.x, w.y));
    }
    function onDrop(e: DragEvent) {
      const type = e.dataTransfer?.getData(MIME);
      if (!type) return;
      e.preventDefault();
      const w = toWorld(e.clientX, e.clientY);
      if (!w) return;
      const p = snapPoint(w.x, w.y);
      const s = useSceneStore.getState();
      s.placeShape(type, p.x, p.y);
      s.armType(null);
      setGhost(null);
    }
    el.addEventListener("dragover", onDragOver);
    el.addEventListener("drop", onDrop);
    return () => {
      el.removeEventListener("dragover", onDragOver);
      el.removeEventListener("drop", onDrop);
    };
  }, [gl, camera]);

  function onClick(e: ThreeEvent<MouseEvent>) {
    const s = useSceneStore.getState();
    if (!s.armedType) {
      s.clearSelection(); // click on empty canvas clears (builder-ux §select)
      return;
    }
    const p = snapPoint(e.point.x, e.point.y);
    if (chaining) {
      // dblclick delivers click-click-dblclick; consecutive-equal dedupe absorbs it
      setChain((prev) => {
        const last = prev[prev.length - 1];
        return last && last[0] === p.x && last[1] === p.y ? prev : [...prev, [p.x, p.y]];
      });
    } else {
      s.placeShape(s.armedType, p.x, p.y);
      s.armType(null);
      setGhost(null);
    }
  }

  function onPointerMove(e: ThreeEvent<PointerEvent>) {
    if (!useSceneStore.getState().armedType) return;
    setGhost(snapPoint(e.point.x, e.point.y));
  }

  const chainPreview: [number, number, number][] = chain.map(([x, y]) => [x, y, 1]);
  if (chaining && ghost && chainPreview.length > 0) chainPreview.push([ghost.x, ghost.y, 1]);

  return (
    <group>
      <mesh
        position={[0, 0, 0]}
        onClick={onClick}
        onDoubleClick={() => chaining && finishChain(chainRef.current)}
        onPointerMove={onPointerMove}
      >
        <planeGeometry args={[100000, 100000]} />
        <meshBasicMaterial transparent opacity={0} depthWrite={false} />
      </mesh>

      {/* ghost: the REAL symbol + translucent ring; ring grows on an anchor hit —
          the connect affordance (place-and-connect in one gesture) */}
      {armedType && ghost && (
        <group position={[ghost.x, ghost.y, 0.5]}>
          {!chaining && def && ghostShape && (
            <def.component shape={ghostShape} values={{}} selected={false} hovered={false} />
          )}
          <mesh position={[0, 0, 1.5]} material={ghostMaterial()} scale={ghost.anchor ? 1.35 : 1}>
            <ringGeometry args={[9, 12, 32]} />
          </mesh>
        </group>
      )}

      {/* chain tool live preview: clicked points + rubber-band to the cursor */}
      {chaining && chainPreview.length >= 2 && (
        <Line points={chainPreview} color={tokens.color.accent} lineWidth={2} transparent opacity={0.75} />
      )}
      {chaining &&
        chain.map(([x, y], i) => (
          <mesh key={i} position={[x, y, 1.2]} material={ghostMaterial()}>
            <circleGeometry args={[3.5, 16]} />
          </mesh>
        ))}
    </group>
  );
}
