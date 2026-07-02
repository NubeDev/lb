// `applyPatch` — pure fold of one typed `Patch` into an `IrSpec`, returning a NEW spec (never mutates).
// The four A2UI-shaped messages: createSurface (replace the whole surface + component map + seed data),
// updateComponents (upsert by id), updateDataModel (set one JSON-Pointer path), deleteSurface (empty
// it). Steady-state data ticks are `updateDataModel` patches — the tree never re-renders structurally.
// This is render-stratum: pure, parser-free.

import type { DataModel, IrSpec, Patch } from "./types";
import { IR_VERSION } from "./types";

/** An empty spec — the starting point a stream folds patches into. */
export function emptySpec(surfaceId = "cell"): IrSpec {
  return { v: IR_VERSION, surface: { surfaceId, root: "" }, components: {} };
}

/** Set `value` at `pointer` in `data`, creating intermediate objects. Returns a new object. */
function setPointer(data: DataModel, pointer: string, value: unknown): DataModel {
  if (pointer === "" || !pointer.startsWith("/")) return data;
  const parts = pointer
    .slice(1)
    .split("/")
    .map((p) => p.replace(/~1/g, "/").replace(/~0/g, "~"));
  const next: DataModel = { ...data };
  let cur: Record<string, unknown> = next;
  for (let i = 0; i < parts.length - 1; i++) {
    const key = parts[i];
    const child = cur[key];
    cur[key] = child && typeof child === "object" && !Array.isArray(child) ? { ...(child as object) } : {};
    cur = cur[key] as Record<string, unknown>;
  }
  cur[parts[parts.length - 1]] = value;
  return next;
}

export function applyPatch(spec: IrSpec, patch: Patch): IrSpec {
  switch (patch.type) {
    case "createSurface":
      return {
        v: spec.v,
        surface: patch.surface,
        components: { ...patch.components },
        dataModel: patch.dataModel,
      };
    case "updateComponents": {
      const components = { ...spec.components };
      for (const c of patch.components) components[c.id] = c;
      return { ...spec, components };
    }
    case "updateDataModel":
      return { ...spec, dataModel: setPointer(spec.dataModel ?? {}, patch.pointer, patch.value) };
    case "deleteSurface":
      return { v: spec.v, surface: { surfaceId: patch.surfaceId, root: "" }, components: {} };
    default: {
      const _exhaustive: never = patch;
      return _exhaustive;
    }
  }
}
