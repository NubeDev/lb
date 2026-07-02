// @vitest-environment happy-dom
// Render tests (thecrew-scope.md testing plan): the scene graph built from
// ahu-demo contains the expected nodes; unknown type renders the placeholder;
// the selection halo appears on select. r3f test renderer — no WebGL pixels in CI,
// and no fakes: the REAL components, store, and simulator run (rule 9).

import ReactThreeTestRenderer from "@react-three/test-renderer";
import { afterEach, describe, expect, it } from "vitest";
import { ahuDemo } from "../scene/demo/ahu-demo";
import { floorplanDemo } from "../scene/demo/floorplan-demo";
import { validateScene } from "../scene/validate";
import { useSceneStore } from "../state/scene-store";
import { haloMaterial } from "../theme/materials";
import { ShapeNode, SYMBOLS } from "./ShapeNode";

function SceneUnderTest({ doc }: { doc: typeof ahuDemo }) {
  return (
    <>
      {Object.entries(doc.shapes).map(([id, shape]) => (
        <ShapeNode key={id} id={id} shape={shape} />
      ))}
    </>
  );
}

afterEach(() => {
  useSceneStore.getState().loadDemo("blank");
  useSceneStore.getState().clearSelection();
});

describe("scene graph rendering", () => {
  it("registry covers every type both demos use", () => {
    for (const doc of [ahuDemo, floorplanDemo]) {
      for (const [id, shape] of Object.entries(doc.shapes)) {
        expect(SYMBOLS[shape.type], `${id}: ${shape.type}`).toBeDefined();
      }
    }
  });

  it("the AHU demo builds its full scene graph (validated, real components)", async () => {
    const { doc } = validateScene(ahuDemo);
    const renderer = await ReactThreeTestRenderer.create(<SceneUnderTest doc={doc} />);
    // one positioned group per shape (always mounted, even while a label's SDF
    // font is still loading behind the per-shape Suspense)...
    expect(renderer.scene.children).toHaveLength(Object.keys(doc.shapes).length);
    // ...and real mesh content from the font-free symbols (ducts alone beat this)
    expect(renderer.scene.findAllByType("Mesh").length).toBeGreaterThan(5);
    await renderer.unmount();
  });

  it("the floor-plan demo renders too", async () => {
    const { doc } = validateScene(floorplanDemo);
    const renderer = await ReactThreeTestRenderer.create(<SceneUnderTest doc={doc} />);
    expect(renderer.scene.findAllByType("Mesh").length).toBeGreaterThan(10);
    await renderer.unmount();
  });

  it("an unknown type renders the labeled placeholder, never crashes", async () => {
    const { doc } = validateScene({
      v: 1,
      camera: "ortho-top",
      shapes: { mystery: { type: "hvac.unobtainium", t: { x: 0, y: 0 }, props: {} } },
    });
    const renderer = await ReactThreeTestRenderer.create(<SceneUnderTest doc={doc} />);
    // placeholder = translucent plane + dashed-feel edge outline (LineSegments)
    expect(renderer.scene.findAllByType("LineSegments").length).toBeGreaterThan(0);
    await renderer.unmount();
  });

  it("selecting a shape adds the halo mesh", async () => {
    const { doc } = validateScene(ahuDemo);
    const renderer = await ReactThreeTestRenderer.create(<SceneUnderTest doc={doc} />);
    const halos = () =>
      renderer.scene
        .findAllByType("Mesh")
        .filter((m) => (m.instance as { material?: unknown }).material === haloMaterial());
    expect(halos()).toHaveLength(0);
    await ReactThreeTestRenderer.act(async () => {
      useSceneStore.getState().select(["sf1"]);
    });
    expect(halos()).toHaveLength(1);
    await renderer.unmount();
  });
});
