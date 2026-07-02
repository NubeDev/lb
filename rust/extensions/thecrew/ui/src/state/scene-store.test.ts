// scene-store unit tests: every completed gesture is ONE undo step, depth ≥50,
// nothing un-undoable (builder-ux-scope.md §undo). The store is module-global —
// each test resets via loadDemo("blank").

import { beforeEach, describe, expect, it } from "vitest";
import { useSceneStore } from "./scene-store";

const store = () => useSceneStore.getState();

beforeEach(() => {
  store().loadDemo("blank");
});

describe("scene-store gestures", () => {
  it("placeShape drops defaults at the point, selects it, one undo step", () => {
    const id = store().placeShape("hvac.fan", 40, -16);
    expect(store().doc.shapes[id].type).toBe("hvac.fan");
    expect(store().doc.shapes[id].t).toMatchObject({ x: 40, y: -16 });
    expect(store().selection).toEqual([id]);
    store().undo();
    expect(store().doc.shapes[id]).toBeUndefined();
    store().redo();
    expect(store().doc.shapes[id]).toBeDefined();
  });

  it("ids never collide", () => {
    const a = store().placeShape("hvac.fan", 0, 0);
    const b = store().placeShape("hvac.fan", 8, 8);
    expect(a).not.toBe(b);
  });

  it("setProp / setBind are undoable and non-destructive", () => {
    const id = store().placeShape("hvac.fan", 0, 0);
    store().setProp(id, "label", "SF-9");
    store().setBind(id, "speed", "ahu1.sf1.speed");
    expect(store().doc.shapes[id].props.label).toBe("SF-9");
    expect(store().doc.shapes[id].bind).toEqual({ speed: { channel: "ahu1.sf1.speed" } });
    store().setBind(id, "speed", null);
    expect(store().doc.shapes[id].bind).toBeUndefined();
    store().undo(); // unbind undone
    expect(store().doc.shapes[id].bind).toEqual({ speed: { channel: "ahu1.sf1.speed" } });
  });

  it("deleteSelection removes and is one undo step", () => {
    const a = store().placeShape("hvac.fan", 0, 0);
    const b = store().placeShape("hvac.coil", 64, 0);
    store().select([a, b]);
    store().deleteSelection();
    expect(Object.keys(store().doc.shapes)).toHaveLength(0);
    store().undo();
    expect(Object.keys(store().doc.shapes)).toHaveLength(2);
  });

  it("duplicateSelection offsets copies and selects them", () => {
    const a = store().placeShape("hvac.fan", 0, 0);
    store().select([a]);
    store().duplicateSelection();
    const ids = Object.keys(store().doc.shapes);
    expect(ids).toHaveLength(2);
    const copyId = store().selection[0];
    expect(copyId).not.toBe(a);
    expect(store().doc.shapes[copyId].t.x).toBe(16);
  });

  it("nudge and moveShapes are single steps", () => {
    const a = store().placeShape("hvac.fan", 0, 0);
    store().select([a]);
    store().nudgeSelection(8, 0);
    expect(store().doc.shapes[a].t.x).toBe(8);
    store().moveShapes({ [a]: { x: 80, y: 40 } });
    expect(store().doc.shapes[a].t).toMatchObject({ x: 80, y: 40 });
    store().undo();
    expect(store().doc.shapes[a].t.x).toBe(8);
    store().undo();
    expect(store().doc.shapes[a].t.x).toBe(0);
  });

  it("undo depth is ≥50", () => {
    for (let i = 0; i < 60; i++) store().placeShape("hvac.fan", i * 8, 0);
    let undone = 0;
    while (useSceneStore.getState().past.length > 0) {
      store().undo();
      undone++;
    }
    expect(undone).toBeGreaterThanOrEqual(50);
  });

  it("a new gesture clears the redo branch", () => {
    store().placeShape("hvac.fan", 0, 0);
    store().undo();
    store().placeShape("hvac.coil", 0, 0);
    expect(useSceneStore.getState().future).toEqual([]);
  });

  it("camera toggle + demo switching work (not undo steps)", () => {
    expect(store().doc.camera).toBe("ortho-top");
    store().toggleCamera();
    expect(store().doc.camera).toBe("persp");
    store().loadDemo("ahu");
    expect(Object.keys(store().doc.shapes).length).toBeGreaterThan(5);
    expect(useSceneStore.getState().past).toEqual([]);
  });
});
