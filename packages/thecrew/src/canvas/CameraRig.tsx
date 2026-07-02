// ortho-top (flat drawing surface, orbit LOCKED) ↔ persp (3D), one ~600 ms spring
// (look-scope.md §motion). The scene must feel like the same object seen differently.
//
// PRAGMATIC v1 (documented tradeoff, phase-4 polish): a true ortho↔persp morph
// needs projection-matrix interpolation. Instead BOTH drei cameras stay mounted
// and `makeDefault` picks the active one; the *transition* is animated on the
// perspective camera only. Entering persp: the persp camera becomes default at a
// near-top-down pose (0, -0.001, 600 — visually the same view) and springs down
// to (0, -420, 380). Entering flat: the persp camera springs back to top-down,
// then we cut to the real ortho camera (poses match, so the cut is invisible).
// The spring is a critically-damped exponential smooth on a single progress
// scalar — no overshoot, no easing gimmicks. Residual v1 seams: persp fov can't
// exactly match the ortho frustum at every viewport size (small framing pop at
// the flat↔transition boundary), and any user pan/zoom is re-centered on origin
// by the toggle. Flat mode itself is exact and perfect NOW; the morph fidelity
// is phase-4. prefers-reduced-motion turns the transition into an instant cut.

import { useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { useFrame } from "@react-three/fiber";
import {
  MapControls,
  OrbitControls,
  OrthographicCamera,
  PerspectiveCamera,
} from "@react-three/drei";
import { useSceneStore } from "../state/scene-store";
import { tokens } from "../theme/tokens";

const REDUCED_MOTION =
  typeof window !== "undefined" &&
  typeof window.matchMedia === "function" &&
  window.matchMedia("(prefers-reduced-motion: reduce)").matches;

/** Top-down-ish start pose: -0.001 in y keeps lookAt well-defined with up=(0,0,1)
 * while reading as exactly top-down, so it matches the ortho camera's view. */
const FLAT_POS = new THREE.Vector3(0, -0.001, 600);
/** 3D pose: pulled back in -Y, raised in +Z, looking at the origin. */
const PERSP_POS = new THREE.Vector3(0, -420, 380);

/** Exponential-smooth rate: reaches ~98% of the step in cameraSpringMs (λt = 4). */
const LAMBDA_PER_SEC = 4000 / tokens.motion.cameraSpringMs;

/** Place the perspective camera at spring progress t ∈ [0..1] (flat → persp). */
function applyPose(cam: THREE.PerspectiveCamera, t: number): void {
  cam.up.set(0, 0, 1); // +Z-up world (SceneCanvas coordinate convention)
  cam.position.lerpVectors(FLAT_POS, PERSP_POS, t);
  cam.lookAt(0, 0, 0);
}

export function CameraRig() {
  const mode = useSceneStore((s) => s.doc.camera);
  const perspRef = useRef<THREE.PerspectiveCamera>(null);
  /** spring progress: 0 = flat/top-down pose, 1 = full persp pose */
  const progress = useRef(mode === "persp" ? 1 : 0);
  /** which camera holds makeDefault; persp also owns the whole transition */
  const [active, setActive] = useState<"ortho" | "persp">(
    mode === "persp" ? "persp" : "ortho",
  );
  const [animating, setAnimating] = useState(false);

  useEffect(() => {
    const target = mode === "persp" ? 1 : 0;
    if (progress.current === target && !animating) return; // mount / no-op toggle
    if (REDUCED_MOTION) {
      // instant cut (look-scope §motion)
      progress.current = target;
      if (perspRef.current) applyPose(perspRef.current, target);
      setAnimating(false);
      setActive(target === 1 ? "persp" : "ortho");
      return;
    }
    // both directions play out on the persp camera; hand it the default now —
    // at progress 0 its view matches the ortho camera, so no visible jump
    setActive("persp");
    setAnimating(true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mode]);

  useFrame((_, rawDt) => {
    if (!animating) return;
    const cam = perspRef.current;
    if (!cam) return;
    const target = mode === "persp" ? 1 : 0;
    const dt = Math.min(rawDt, 0.1); // clamp tab-switch spikes
    const k = 1 - Math.exp(-LAMBDA_PER_SEC * dt);
    const next = progress.current + (target - progress.current) * k;
    progress.current = Math.abs(next - target) < 0.002 ? target : next;
    applyPose(cam, progress.current);
    if (progress.current === target) {
      setAnimating(false);
      // arrived back at top-down → cut to the true ortho camera (invisible)
      if (target === 0) setActive("ortho");
    }
  });

  return (
    <>
      {/* flat: true orthographic top-down. Looking down -Z, up=(0,1,0) puts
          world +Y at screen-up. zoom 1.6 frames ~±350 world units. */}
      <OrthographicCamera
        makeDefault={active === "ortho"}
        position={[0, 0, 600]}
        up={[0, 1, 0]}
        zoom={1.6}
        near={0.1}
        far={2000}
      />
      <PerspectiveCamera
        ref={perspRef}
        makeDefault={active === "persp"}
        position={mode === "persp" ? PERSP_POS.toArray() : FLAT_POS.toArray()}
        up={[0, 0, 1]}
        fov={45}
        near={1}
        far={5000}
      />

      {active === "ortho" ? (
        // flat mode: pan + zoom only, orbit LOCKED (builder-ux)
        <MapControls
          enableRotate={false}
          screenSpacePanning
          minZoom={0.4}
          maxZoom={6}
          target={[0, 0, 0]}
        />
      ) : (
        // 3D mode: orbit, but never under the floor. Disabled mid-spring so the
        // controls don't fight the animated camera.
        <OrbitControls
          enabled={!animating}
          maxPolarAngle={Math.PI / 2 - 0.08}
          minDistance={80}
          maxDistance={2000}
          target={[0, 0, 0]}
        />
      )}
    </>
  );
}
