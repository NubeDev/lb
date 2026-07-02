// Auto-fit ortho camera for a read-only cell (thecrew parent-scope risk: "fit per dashboard cell").
// Mounted INSIDE the <Canvas> (it needs the r3f `size` + `camera`). It reframes the flat ortho camera
// to the scene's bounding box in the actual cell pixels + pans to the scene center, so equipment sitting
// away from the origin is visible in a small grid cell (the editor's fixed ±350 / zoom 1.6 crop rendered
// blank). It ONLY drives the orthographic camera in flat mode — the persp/orbit path (editor) is
// untouched, so this is additive and page framing is unchanged.
//
// The fit is applied EVERY FRAME (a handful of float ops), not once in an effect: drei's
// <OrthographicCamera makeDefault> re-applies its declarative `zoom`/`position` props on re-render, so a
// one-shot effect gets clobbered back to the page default (the "still blank" symptom). Owning the pose in
// useFrame makes FitCamera authoritative and also tracks cell resize/scene edits for free.

import { useMemo } from "react";
import * as THREE from "three";
import { useFrame, useThree } from "@react-three/fiber";
import { useSceneStore } from "../state/scene-store";
import { sceneBounds, fitZoom, boundsCenter } from "./fit-bounds";

/** Reframe the active ortho camera to the scene bounds + the current viewport, every frame. A no-op when
 *  the default camera isn't orthographic (persp/3D mode owns its own framing). */
export function FitCamera() {
  const doc = useSceneStore((s) => s.doc);
  const size = useThree((s) => s.size);
  // Bounds change only when the doc does — recompute lazily, then reuse per frame.
  const bounds = useMemo(() => sceneBounds(doc), [doc]);
  const center = useMemo(() => boundsCenter(bounds), [bounds]);

  useFrame(({ camera }) => {
    if (!(camera instanceof THREE.OrthographicCamera)) return;
    const zoom = fitZoom(bounds, size.width, size.height);
    // Only touch the camera when the frame differs — avoids a needless projection-matrix rebuild.
    if (camera.zoom !== zoom || camera.position.x !== center.x || camera.position.y !== center.y) {
      camera.zoom = zoom;
      camera.position.set(center.x, center.y, camera.position.z || 600);
      camera.up.set(0, 1, 0);
      camera.lookAt(center.x, center.y, 0);
      camera.updateProjectionMatrix();
    }
  });

  return null;
}
