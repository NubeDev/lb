// The GenUI internal representation (IR) — the versioned, A2UI-*shaped* contract a cell PERSISTS and a
// viewer CONSUMES (genui-scope "Why the IR is A2UI-shaped"). Implemented in-house: the message *shapes*
// stay compatible with A2UI v0.9 so an adapter can be slotted in later, but we depend on none of
// Google's packages. Four load-bearing patterns are adopted:
//
//   - Surface        — one addressable render root (`surfaceId`); here, one cell.
//   - Flat component  — `{id, component, props, children:[ids]}`; easy for an LLM to emit and patch,
//     map              cheap to validate, orphan-tolerant.
//   - JSON-Pointer    — components BIND paths (`{"$bind":"/data/A/latest"}`) instead of embedding
//     data model       values, so a data tick is a data-model patch, never a tree re-render.
//   - Typed messages  — createSurface | updateComponents | updateDataModel | deleteSurface in;
//                       `action {surfaceId, componentId, name, context}` out.
//
// This file is TYPES ONLY (no logic, no parsers) — it is in the RENDER stratum every viewer loads.

/** The current IR schema version. `migrate.ts` upgrades older persisted specs forward on load. */
export const IR_VERSION = 1 as const;

/** A binding to a value in the surface's data model, addressed by JSON Pointer (RFC 6901).
 *  A component prop is either a literal or one of these — `resolveBindings` swaps `$bind` for the
 *  pointed-at value at render time. `$bind` (not bare `{path}`) so a literal object prop can never be
 *  mistaken for a binding. */
export interface Binding {
  $bind: string;
}

export function isBinding(v: unknown): v is Binding {
  return typeof v === "object" && v !== null && typeof (v as Binding).$bind === "string";
}

/** A prop value: any JSON literal, a binding, or a (possibly nested) array/object of the same. */
export type PropValue =
  | string
  | number
  | boolean
  | null
  | Binding
  | PropValue[]
  | { [k: string]: PropValue };

/** One component instance in the flat map. `children` names other component ids (never inline nodes),
 *  so the tree is id-referenced and orphan-tolerant. `component` must resolve in the catalog. */
export interface Component {
  id: string;
  component: string;
  props?: Record<string, PropValue>;
  children?: string[];
}

/** The addressable render root. `root` names the top component id; the tree is walked from there. */
export interface Surface {
  surfaceId: string;
  root: string;
}

/** The per-surface data model: an arbitrary JSON tree that bindings point into. The dashboard tenant
 *  keys it by refId under `/data/{refId}` (e.g. `/data/A/rows`), but the IR itself is agnostic. */
export type DataModel = Record<string, unknown>;

/** The whole persisted spec: version + surface + the flat component map + an OPTIONAL seed data model
 *  (authoring-time sample; steady-state data arrives via patches, never persisted). */
export interface IrSpec {
  v: number;
  surface: Surface;
  components: Record<string, Component>;
  dataModel?: DataModel;
}

/** A typed action a control emits back over the bridge — `name` is the catalog action, `tool` the
 *  MCP verb the host re-checks against the cell leash, `context` the resolved args. */
export interface UiAction {
  surfaceId: string;
  componentId: string;
  name: string;
  tool: string;
  context?: Record<string, unknown>;
}

/** The four typed patch messages (A2UI-shaped). `applyPatch` folds one into an `IrSpec`. */
export type Patch =
  | { type: "createSurface"; surface: Surface; components: Record<string, Component>; dataModel?: DataModel }
  | { type: "updateComponents"; components: Component[] }
  | { type: "updateDataModel"; pointer: string; value: unknown }
  | { type: "deleteSurface"; surfaceId: string };

/** A single validate/normalize finding surfaced to the AUTHOR (never a viewer). `level:"error"` blocks
 *  accept; `level:"warning"` is a fixed-up sloppiness the author is shown before saving. */
export interface Finding {
  level: "warning" | "error";
  code: string;
  message: string;
  componentId?: string;
}
