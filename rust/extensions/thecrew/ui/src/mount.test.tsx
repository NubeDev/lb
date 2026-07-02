// @vitest-environment happy-dom
// Federation-contract tests: the page and the widget both mount through the exported
// `mountPage`/`mountWidget` (the frozen shell handshake), and both unmount cleanly (hot-reload
// safe — stateless). Deny paths surface honestly. Backed by the bridge stub; the REAL host
// re-check + workspace wall live in the gateway suite (ui/).

import { describe, expect, it } from "vitest";
import { mountPage, mountWidget } from "./remoteEntry";
import { stubBridge, rejectingBridge, watchBridge } from "./bridge/bridge.stub";
import { serializeScene } from "./bridge/scene-io";
import type { SceneDoc } from "./scene/scene.types";

const SCENE: SceneDoc = {
  v: 1,
  camera: "ortho-top",
  shapes: { sf1: { type: "hvac.fan", t: { x: 0, y: 0 }, props: { label: "SF-1" }, bind: { speed: { channel: "s.speed" } } } },
};

/** Let React 18's async render + effects + the bridge's async load/list promises flush. Several
 *  turns: effect fires → bridge promise resolves → setState → re-render → effect again. */
async function flush() {
  for (let i = 0; i < 6; i++) {
    await new Promise((r) => setTimeout(r, 0));
    await Promise.resolve();
  }
}

describe("mountPage (the [ui] page)", () => {
  it("mounts through the federation entry and lists scenes, then unmounts cleanly", async () => {
    const el = document.createElement("div");
    const bridge = stubBridge({
      "assets.list_docs": () => ({ docs: [{ id: "scene:ahu", title: "AHU-1" }] }),
      "series.latest": () => ({ sample: null }),
    });
    const unmount = mountPage(el, { workspace: "acme" }, bridge);
    await flush();
    expect(el.querySelector('[data-testid="scene-persistence-bar"]')).toBeTruthy();
    // the picker offers the seeded scene
    expect(el.textContent).toContain("AHU-1");
    unmount();
    expect(el.innerHTML).toBe("");
  });

  it("a denied list_docs leaves the page usable (empty picker), never a crash", async () => {
    const el = document.createElement("div");
    const unmount = mountPage(el, { workspace: "acme" }, rejectingBridge());
    await flush();
    expect(el.querySelector('[data-testid="scene-persistence-bar"]')).toBeTruthy();
    unmount();
  });
});

describe("mountWidget (the [[widget]] cell)", () => {
  it("renders a read-only scene cell for a scene id (no save button)", async () => {
    const el = document.createElement("div");
    const { bridge } = watchBridge({
      "assets.get_doc": () => ({ id: "scene:ahu", title: "AHU-1", content: serializeScene(SCENE) }),
      "series.latest": () => ({ sample: { payload: 880 } }),
    });
    const unmount = mountWidget(
      el,
      { workspace: "acme", binding: {}, options: { sceneId: "scene:ahu" } },
      bridge,
      "scene",
    );
    await flush();
    expect(el.querySelector('[data-testid="scene-widget"]')).toBeTruthy();
    // the cell can NEVER save — no persistence bar / save button on the widget
    expect(el.querySelector('[data-testid="scene-save"]')).toBeNull();
    unmount();
    expect(el.innerHTML).toBe("");
  });

  it("a viewer denied the scene sees an honest empty state, not a crash", async () => {
    const el = document.createElement("div");
    const unmount = mountWidget(
      el,
      { workspace: "acme", binding: {}, options: { sceneId: "scene:secret" } },
      rejectingBridge() as never,
      "scene",
    );
    await flush();
    expect(el.querySelector('[data-testid="scene-widget-empty"]')).toBeTruthy();
    unmount();
  });
});
