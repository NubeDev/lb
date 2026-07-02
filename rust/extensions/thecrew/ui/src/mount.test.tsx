// @vitest-environment happy-dom
// Federation-contract tests: the page and the widget both mount through the exported
// `mountPage`/`mountWidget` (the frozen shell handshake), and both unmount cleanly (hot-reload
// safe — stateless). Deny paths surface honestly. Backed by the bridge stub; the REAL host
// re-check + workspace wall live in the gateway suite (ui/).

import { describe, expect, it } from "vitest";
import * as remoteEntry from "./remoteEntry";
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

// Finding 6: the shell's `pickMount` resolves the PAGE by the frozen export name **`mount`** — thecrew
// once exported only `mountPage`, so the live shell threw "remote does not export a `mount` function"
// and the page never mounted. The unit suite imports `mountPage` directly, so it was blind to the name.
// This asserts the built module exposes the frozen names, failing fast without a browser (the honest
// guard alongside the live-shell e2e). A new bundling extension gets the same cheap check by copying it.
describe("federation export contract (finding 6)", () => {
  it("exports a `mount` function (the name the shell's pickMount resolves the PAGE by)", () => {
    expect(typeof remoteEntry.mount).toBe("function");
  });
  it("exports a `mountWidget` function (the dashboard cell contract)", () => {
    expect(typeof remoteEntry.mountWidget).toBe("function");
  });
  it("the default export also carries both (the object-form fallback pickMount accepts)", () => {
    expect(typeof remoteEntry.default.mount).toBe("function");
    expect(typeof remoteEntry.default.mountWidget).toBe("function");
  });
});

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
