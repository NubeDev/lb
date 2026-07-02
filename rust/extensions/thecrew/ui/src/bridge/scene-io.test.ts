// scene-io unit tests: the assets.* seam contract, the scene-id/tag convention, and the
// last-writer-wins interim (read-before-write conflict). Backed by the bridge stub (the frontend's
// view); the REAL host re-check + workspace wall is exercised in the gateway suite (ui/).

import { describe, expect, it, vi } from "vitest";
import { stubBridge } from "./bridge.stub";
import {
  listScenes,
  loadScene,
  saveScene,
  sceneId,
  serializeScene,
  SceneConflictError,
  SCENE_PREFIX,
  SCENE_TAG,
} from "./scene-io";
import type { SceneDoc } from "../scene/scene.types";

const DOC: SceneDoc = {
  v: 1,
  camera: "ortho-top",
  shapes: { sf1: { type: "hvac.fan", t: { x: 0, y: 0 }, props: {}, bind: { speed: { channel: "s.speed" } } } },
};

describe("scene id convention", () => {
  it("prefixes ids idempotently", () => {
    expect(sceneId("ahu-1")).toBe(`${SCENE_PREFIX}ahu-1`);
    expect(sceneId(`${SCENE_PREFIX}ahu-1`)).toBe(`${SCENE_PREFIX}ahu-1`);
  });
});

describe("listScenes", () => {
  it("filters list_docs down to scene-prefixed docs (list carries no tags)", async () => {
    const bridge = stubBridge({
      "assets.list_docs": () => ({
        docs: [
          { id: `${SCENE_PREFIX}ahu`, title: "AHU-1" },
          { id: "note:readme", title: "not a scene" },
        ],
      }),
    });
    const scenes = await listScenes(bridge);
    expect(scenes).toEqual([{ id: `${SCENE_PREFIX}ahu`, title: "AHU-1" }]);
  });
});

describe("loadScene", () => {
  it("parses + normalizes the stored JSON and keeps a byte baseline", async () => {
    const bridge = stubBridge({
      "assets.get_doc": () => ({ id: `${SCENE_PREFIX}ahu`, title: "AHU-1", content: serializeScene(DOC) }),
    });
    const loaded = await loadScene(bridge, "ahu");
    expect(loaded.id).toBe(`${SCENE_PREFIX}ahu`);
    expect(loaded.doc.shapes.sf1.type).toBe("hvac.fan");
    expect(loaded.baseline).toBe(serializeScene(DOC));
  });

  it("never crashes on a corrupt body — empty scene", async () => {
    const bridge = stubBridge({
      "assets.get_doc": () => ({ id: `${SCENE_PREFIX}x`, title: "x", content: "{not json" }),
    });
    const loaded = await loadScene(bridge, "x");
    expect(loaded.doc.shapes).toEqual({});
  });
});

describe("saveScene", () => {
  it("writes put_doc with content_type json + the scene tag", async () => {
    const put = vi.fn(() => ({ id: `${SCENE_PREFIX}ahu` }));
    const bridge = stubBridge({ "assets.put_doc": put });
    await saveScene(bridge, { id: "ahu", title: "AHU-1", doc: DOC });
    expect(put).toHaveBeenCalledWith(
      expect.objectContaining({
        id: `${SCENE_PREFIX}ahu`,
        title: "AHU-1",
        content_type: "json",
        tags: [SCENE_TAG],
      }),
    );
  });

  it("last-writer-wins interim: throws SceneConflictError when the store changed underneath", async () => {
    // The editor loaded baseline = DOC; the store now holds a DIFFERENT doc → conflict, no clobber.
    const changed: SceneDoc = { ...DOC, shapes: {} };
    const put = vi.fn(() => ({ id: `${SCENE_PREFIX}ahu` }));
    const bridge = stubBridge({
      "assets.get_doc": () => ({ id: `${SCENE_PREFIX}ahu`, title: "AHU-1", content: serializeScene(changed) }),
      "assets.put_doc": put,
    });
    const loaded = { id: `${SCENE_PREFIX}ahu`, title: "AHU-1", doc: DOC, baseline: serializeScene(DOC) };
    await expect(saveScene(bridge, { id: "ahu", title: "AHU-1", doc: DOC, loaded })).rejects.toBeInstanceOf(
      SceneConflictError,
    );
    expect(put).not.toHaveBeenCalled(); // never clobbers on conflict
  });

  it("saves when the store still matches the loaded baseline", async () => {
    const put = vi.fn(() => ({ id: `${SCENE_PREFIX}ahu` }));
    const bridge = stubBridge({
      "assets.get_doc": () => ({ id: `${SCENE_PREFIX}ahu`, title: "AHU-1", content: serializeScene(DOC) }),
      "assets.put_doc": put,
    });
    const loaded = { id: `${SCENE_PREFIX}ahu`, title: "AHU-1", doc: DOC, baseline: serializeScene(DOC) };
    const next = await saveScene(bridge, { id: "ahu", title: "AHU-1", doc: DOC, loaded });
    expect(put).toHaveBeenCalledOnce();
    expect(next.baseline).toBe(serializeScene(DOC));
  });

  it("a denied save (no put_doc grant) surfaces out_of_scope — never silent", async () => {
    const bridge = stubBridge({}); // no assets.put_doc in scope
    await expect(saveScene(bridge, { id: "ahu", title: "AHU-1", doc: DOC })).rejects.toThrow(/out_of_scope/);
  });
});
