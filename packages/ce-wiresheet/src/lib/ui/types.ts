// Declarative UI types — the high-level Collection/View DSL the wiresheet's tab
// host renders (see ../../SDUI_UNIFIED_DESIGN.md §10). This is the *authoring*
// shape an extension ships (or the engine serves at GET /ui/list); it compiles
// down to renderer widgets. Kept deliberately small for the first slice —
// `collection` + `layout` only, widgets loosely typed so the renderer registry
// can grow without churning this file.

/** Unifies CE `__facets` and collection `fields` — one descriptor for a column
 *  or a form editor, regardless of whether it came from `/schema` + `__facets`
 *  (a CE component's prop) or a hand-declared collection field. */
export type FieldType = "string" | "number" | "boolean" | "enum" | "datetime";

export interface FieldDescriptor {
  name: string;
  type: FieldType;
  label?: string;
  /** schema-level */
  readonly?: boolean;
  required?: boolean;
  primary?: boolean;
  /** enum choices */
  values?: { label: string; value: string | number | boolean }[];
  /** presentation (from `__facets`) */
  unit?: string;
  decimals?: number;
  hidden?: boolean;
  order?: number;
}

/** How a UI relates to the host's component selection (graph ⇄ drawer). */
export type SelectionMode = "ignore" | "follow" | "drive" | "sync";

/** Scope an action applies to (from the alarms DSL). */
export type ActionTarget = "record" | "selection" | "collection" | "component";

export interface ActionDef {
  name: string;
  label: string;
  target: ActionTarget;
  /** confirmation prompt for destructive actions */
  confirm?: string;
  /** navigate to another view id */
  link?: string;
  /** resolved params to send with the action (e.g. a widget's staged edits) */
  params?: Record<string, unknown>;
}

/** A tabular view of a collection (e.g. the components table). Columns default
 *  to the source's visible field descriptors when omitted. */
export interface CollectionView {
  type: "collection";
  /** named collection the host resolves — e.g. "components" (current folder). */
  source: string;
  columns?: string[];
  fullBleed?: boolean;
  multiselect?: boolean;
  actions?: ActionDef[];
}

/** A free-form composed panel (rows/values/buttons/custom widgets). */
export interface LayoutView {
  type: "layout";
  children: Widget[];
  /** optional grid of view/widget ids for dashboard-style composition */
  grid?: string[][];
}

/** A single-record detail/edit view. */
export interface RecordView {
  type: "record";
  source: string;
  fields?: string[];
}

/** The folder hierarchy as an expandable tree with per-folder counts. */
export interface TreeView {
  type: "tree";
  source: string;
  fullBleed?: boolean;
}

export type View = CollectionView | LayoutView | RecordView | TreeView;

/** Widgets inside a `layout` view. Open-ended: `type` is resolved through the
 *  renderer registry, so new widgets (gauge, schedule, …) need no change here. */
export interface Widget {
  type: string;
  /** bind to live data: a CE prop name, or an opaque subject handle */
  bind?: { prop?: string; subject?: string };
  label?: string;
  text?: string;
  action?: ActionDef;
  [k: string]: unknown;
}

/** One tab in the drawer. */
export interface UiEntry {
  id: string;
  label: string;
  /** lucide icon name */
  icon?: string;
  selection: SelectionMode;
  /** component type this UI is for (follow/sync bind only to a matching
   *  selection, e.g. "schedule"). Omit for type-agnostic UIs. */
  appliesTo?: string;
  /** list this type's components across ALL folders in the empty-state picker
   *  (depth-independent `?type=` scan), not just the current folder. */
  global?: boolean;
  /** full "vendor-ext::name" type for the global scan when no instance of the
   *  type exists in the current folder to infer it from. */
  fullType?: string;
  view: View;
}

/** What GET /ui/list returns (and what the stub mirrors). */
export interface UiManifest {
  version: number;
  uis: UiEntry[];
}

/** One loaded extension's contribution to the drawer: a right-edge tab whose
 *  inner side-strip is this extension's UIs. The drawer shows one extension at a
 *  time; switching extensions is the outer (right-edge) tab level. */
export interface ExtensionUi {
  id: string;
  label: string;
  /** right-edge tab icon (lucide name) */
  icon?: string;
  uis: UiEntry[];
}
