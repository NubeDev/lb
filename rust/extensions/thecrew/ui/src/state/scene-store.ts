// The zustand store: scene doc + selection + camera mode + undo stack.
// Single writer for the document; editor hooks dispatch through here so every
// completed gesture is one undo step (builder-ux-scope.md §undo).
// Undo is snapshot-based over the doc (docs are small; ≥50 depth kept).

import { create } from "zustand";
import type { SceneDoc, SceneShape, Transform } from "../scene/scene.types";
import { validateScene } from "../scene/validate";
import { defaultShape } from "../scene/defaults";
import { ahuDemo } from "../scene/demo/ahu-demo";
import { floorplanDemo } from "../scene/demo/floorplan-demo";

export type DemoName = "ahu" | "plan" | "blank";
export type Tool = "select" | "chain";

const UNDO_DEPTH = 64; // builder-ux-scope §undo: ≥50

const DEMOS: Record<DemoName, () => SceneDoc> = {
  ahu: () => validateScene(ahuDemo).doc,
  plan: () => validateScene(floorplanDemo).doc,
  blank: () => ({ v: 1, camera: "ortho-top", shapes: {} }),
};

function clone(doc: SceneDoc): SceneDoc {
  return structuredClone(doc);
}

function freshId(doc: SceneDoc, type: string): string {
  const base = type.split(".").pop() ?? "shape";
  for (let n = 1; ; n++) {
    const id = `${base}-${n}`;
    if (!(id in doc.shapes)) return id;
  }
}

export interface SceneStore {
  doc: SceneDoc;
  demo: DemoName;
  selection: string[];
  hovered: string | null;
  snapEnabled: boolean;
  tool: Tool;
  /** palette click-to-arm / drag payload: the type the next canvas click places */
  armedType: string | null;
  past: SceneDoc[];
  future: SceneDoc[];

  // document mutations — each is ONE undo step
  placeShape: (type: string, x: number, y: number, overrides?: Partial<SceneShape>) => string;
  addShape: (id: string, shape: SceneShape) => void;
  moveShapes: (moves: Record<string, Partial<Transform>>) => void;
  setProp: (id: string, key: string, value: unknown) => void;
  setBind: (id: string, prop: string, channel: string | null) => void;
  deleteSelection: () => void;
  duplicateSelection: () => void;
  nudgeSelection: (dx: number, dy: number) => void;

  // ephemeral (not undoable)
  select: (ids: string[], additive?: boolean) => void;
  clearSelection: () => void;
  setHovered: (id: string | null) => void;
  toggleSnap: () => void;
  toggleCamera: () => void;
  setTool: (tool: Tool) => void;
  armType: (type: string | null) => void;
  loadDemo: (demo: DemoName) => void;
  /** Load an externally-sourced doc (a scene fetched over the bridge) as the current document,
   *  clearing history. Additive to the playground store — the lift needs a way to inject a
   *  persisted scene the playground never had (thecrew-extension-scope.md §UI lift). */
  loadDoc: (doc: SceneDoc) => void;

  undo: () => void;
  redo: () => void;
}

export const useSceneStore = create<SceneStore>((set, get) => {
  /** Apply a doc mutation as one undo step. */
  function commit(mutate: (doc: SceneDoc) => void) {
    const { doc, past } = get();
    const next = clone(doc);
    mutate(next);
    set({
      doc: next,
      past: [...past.slice(-(UNDO_DEPTH - 1)), doc],
      future: [],
    });
  }

  return {
    doc: DEMOS.ahu(),
    demo: "ahu",
    selection: [],
    hovered: null,
    snapEnabled: true,
    tool: "select",
    armedType: null,
    past: [],
    future: [],

    placeShape(type, x, y, overrides) {
      let id = "";
      commit((doc) => {
        id = freshId(doc, type);
        const shape = { ...defaultShape(type), ...overrides };
        shape.t = { ...shape.t, ...overrides?.t, x, y };
        doc.shapes[id] = shape;
      });
      set({ selection: [id] });
      return id;
    },

    addShape(id, shape) {
      commit((doc) => {
        doc.shapes[id] = shape;
      });
    },

    moveShapes(moves) {
      commit((doc) => {
        for (const [id, t] of Object.entries(moves)) {
          const shape = doc.shapes[id];
          if (shape) shape.t = { ...shape.t, ...t };
        }
      });
    },

    setProp(id, key, value) {
      commit((doc) => {
        const shape = doc.shapes[id];
        if (shape) shape.props = { ...shape.props, [key]: value };
      });
    },

    setBind(id, prop, channel) {
      commit((doc) => {
        const shape = doc.shapes[id];
        if (!shape) return;
        const bind = { ...shape.bind };
        if (channel) bind[prop] = { channel };
        else delete bind[prop];
        if (Object.keys(bind).length > 0) shape.bind = bind;
        else delete shape.bind;
      });
    },

    deleteSelection() {
      const { selection } = get();
      if (selection.length === 0) return;
      commit((doc) => {
        for (const id of selection) delete doc.shapes[id];
      });
      set({ selection: [] });
    },

    duplicateSelection() {
      const { selection } = get();
      if (selection.length === 0) return;
      const placed: string[] = [];
      commit((doc) => {
        for (const id of selection) {
          const src = doc.shapes[id];
          if (!src) continue;
          const copy = structuredClone(src);
          copy.t = { ...copy.t, x: copy.t.x + 16, y: copy.t.y - 16 };
          const newId = freshId(doc, copy.type);
          doc.shapes[newId] = copy;
          placed.push(newId);
        }
      });
      if (placed.length > 0) set({ selection: placed });
    },

    nudgeSelection(dx, dy) {
      const { selection } = get();
      if (selection.length === 0) return;
      commit((doc) => {
        for (const id of selection) {
          const shape = doc.shapes[id];
          if (shape) shape.t = { ...shape.t, x: shape.t.x + dx, y: shape.t.y + dy };
        }
      });
    },

    select(ids, additive = false) {
      set((s) => ({
        selection: additive ? [...new Set([...s.selection, ...ids])] : ids,
      }));
    },
    clearSelection: () => set({ selection: [] }),
    setHovered: (id) => set({ hovered: id }),
    toggleSnap: () => set((s) => ({ snapEnabled: !s.snapEnabled })),
    toggleCamera: () =>
      set((s) => ({
        doc: { ...s.doc, camera: s.doc.camera === "ortho-top" ? "persp" : "ortho-top" },
      })),
    setTool: (tool) => set({ tool }),
    armType: (type) => set({ armedType: type, tool: "select" }),
    loadDemo: (demo) =>
      set({ doc: DEMOS[demo](), demo, selection: [], past: [], future: [] }),
    loadDoc: (doc) =>
      // Normalize on the way in (the same total path demos take) so a hand-authored or
      // agent-written scene can never crash the render.
      set({ doc: validateScene(doc).doc, selection: [], past: [], future: [] }),

    undo() {
      const { past, doc, future } = get();
      const prev = past[past.length - 1];
      if (!prev) return;
      set({ doc: prev, past: past.slice(0, -1), future: [doc, ...future], selection: [] });
    },
    redo() {
      const { future, doc, past } = get();
      const next = future[0];
      if (!next) return;
      set({ doc: next, future: future.slice(1), past: [...past, doc], selection: [] });
    },
  };
});
