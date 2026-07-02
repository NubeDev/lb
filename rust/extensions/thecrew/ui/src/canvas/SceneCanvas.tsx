// The r3f <Canvas> + the full look pipeline: soft key light + low ambient, AO,
// SELECTIVE bloom (status emissives + selection halo only — look-scope.md anti-goals),
// ground grid that fades with zoom. Renders the shape map via ShapeNode.
//
// Coordinate convention (holds for both cameras — CameraRig owns them): the ground
// plane is XY and +Z is up. PlaneGeometry is XY-native so shapes/ground need no
// rotation; drei's Grid is XZ-native and is rotated below.

import { useEffect, useState } from "react";
import * as THREE from "three";
import { Canvas, useFrame } from "@react-three/fiber";
import { Grid } from "@react-three/drei";
import { Bloom, EffectComposer, Vignette } from "@react-three/postprocessing";
import { DropPlane } from "../editor/use-drag-place";
import { useSceneStore } from "../state/scene-store";
import { tokens, refreshTokens } from "../theme/tokens";
import { updateMaterials, refreshMaterials } from "../theme/materials";
import { subscribeThemeChange } from "../theme/host-tokens";
import { CameraRig } from "./CameraRig";
import { FitCamera } from "./FitCamera";
import { ShapeNode } from "./ShapeNode";

/** Page-load preference; a module-level check keeps the render path flat
 * (look-scope §motion: reduced motion freezes flow/spin/pulse). */
const REDUCED_MOTION =
  typeof window !== "undefined" &&
  typeof window.matchMedia === "function" &&
  window.matchMedia("(prefers-reduced-motion: reduce)").matches;

/** The ONE scene-wide material tick (fault pulse at ~0.5 Hz — theme/materials.ts).
 * Not mounted at all under prefers-reduced-motion, so status pulses freeze. */
function MaterialsTick() {
  useFrame((state) => updateMaterials(state.clock.elapsedTime));
  return null;
}

/** `fit` (read-only cell): auto-frame the scene into the cell pixels and lock pan/zoom — the editor
 *  page leaves it off and keeps its fixed framing + MapControls. */
export function SceneCanvas({ fit = false }: { fit?: boolean } = {}) {
  const doc = useSceneStore((s) => s.doc);
  const clearSelection = useSceneStore((s) => s.clearSelection);
  // Follow the host theme (light/dark + accent swatch): on a change, re-read the host tokens,
  // drop the cached materials so they rebuild from the new colors, then bump a key to re-render
  // the r3f tree (its <color>/light/Grid props read tokens.color at render time).
  const [themeGen, setThemeGen] = useState(0);
  useEffect(
    () =>
      subscribeThemeChange(() => {
        refreshTokens();
        refreshMaterials();
        setThemeGen((n) => n + 1);
      }),
    [],
  );

  return (
    <Canvas
      style={{ position: "absolute", inset: 0 }}
      dpr={[1, 2]}
      gl={{ antialias: true }}
      flat={false}
      shadows // PCFSoft — soft directional shadows are the depth cue in flat mode
      // click on empty canvas = clear selection (builder-ux)
      onPointerMissed={() => clearSelection()}
    >
      <color attach="background" args={[tokens.color.canvas]} />

      <CameraRig fit={fit} />
      {fit && <FitCamera />}
      {!REDUCED_MOTION && <MaterialsTick />}

      {/* ---- lighting rig (look-scope §recipe) --------------------------------
          Low, slightly cool ambient (tinted with the cool near-white text token —
          no hex outside theme/) + one soft key light from above (+Z), offset in
          x/y so extrusion side-walls catch light and shapes read as depth even
          in the flat top-down view. A faint fill from the opposite quadrant
          keeps shadowed faces from going dead black. */}
      <ambientLight intensity={0.5} color={tokens.color.text.value} />
      <directionalLight
        position={[150, -180, 420]}
        intensity={1.1}
        castShadow
        shadow-mapSize={[2048, 2048]}
        shadow-bias={-0.0004}
        // ortho shadow camera sized to the working area (~±400 world units)
        shadow-camera-left={-400}
        shadow-camera-right={400}
        shadow-camera-top={400}
        shadow-camera-bottom={-400}
        shadow-camera-near={1}
        shadow-camera-far={1200}
      />
      <directionalLight position={[-220, 160, 300]} intensity={0.25} />

      {/* ---- ground -----------------------------------------------------------
          Large canvas-colored plane just below z=0 that receives the key light's
          soft shadows — the grounding/contact cue.

          AO DECISION: no drei <ContactShadows> / SSAO. ContactShadows is
          XZ-native (its internal shadow camera looks down -Y), which fights our
          +Z-up world — wrapping it in a rotated group distorts its blur pass.
          The soft directional shadow above already grounds equipment, and the
          look-scope open question requires the look to survive with AO off, so
          flat mode ships without it. Revisit (rotated group or N8AO) in the
          phase-4 3D pass if tilted views want more under-equipment occlusion. */}
      <mesh position={[0, 0, -2]} receiveShadow>
        <planeGeometry args={[4000, 4000]} />
        <meshStandardMaterial color={tokens.color.canvas} roughness={1} metalness={0} />
      </mesh>

      {/* barely-there grid (look-scope §depth-without-noise). drei Grid is
          XZ-native — rotated +90° about X so its plane is XY (+Z normal),
          sitting just above the ground plane to avoid z-fighting. */}
      <Grid
        position={[0, 0, -1.9]}
        rotation={[Math.PI / 2, 0, 0]}
        cellSize={tokens.grid.step}
        sectionSize={tokens.grid.step * 8}
        cellColor={tokens.color.grid}
        sectionColor={tokens.color.grid}
        cellThickness={0.6}
        sectionThickness={1}
        fadeDistance={1600}
        fadeStrength={1.5}
        infiniteGrid
        followCamera={false}
        side={THREE.DoubleSide}
      />

      {/* ---- the document ---- */}
      {/* keyed by themeGen: a host theme change clears the material cache (refreshMaterials), so the
          shapes must re-mount to rebuild their materials from the new tokens. */}
      <group key={themeGen}>
        {Object.entries(doc.shapes).map(([id, shape]) => (
          <ShapeNode key={id} id={id} shape={shape} />
        ))}
      </group>

      {/* placement gestures: click/drag-drop placement, ghost preview, chain tool
          (editor/use-drag-place.tsx — must live inside the <Canvas>) */}
      <DropPlane />

      {/* ---- post (look-scope §glow) -------------------------------------------
          SELECTIVE bloom via the threshold convention: luminanceThreshold ≥ 1, so
          only materials pushed past it (status emissives with emissiveIntensity
          > 1, the selection halo — see theme/materials.ts) glow. Bodies, ducts,
          grid, text never cross it. No full-scene bloom, no lens flares. */}
      <EffectComposer>
        <Bloom
          mipmapBlur
          intensity={0.9}
          luminanceThreshold={1}
          luminanceSmoothing={0.2}
        />
        {/* very subtle edge falloff — pulls the eye to the center of the wall shot */}
        <Vignette offset={0.25} darkness={0.55} eskil={false} />
      </EffectComposer>
    </Canvas>
  );
}
