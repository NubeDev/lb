// The canonical vocabulary the source picker produces + the injected seam it reads through.
//
// This package is TRANSPORT-AGNOSTIC by design (source-picker-package-scope.md): it never imports an
// API client or `invoke`/`bridge`. The host supplies a `SourceLoaders` — a tiny bag of read functions
// returning the row shapes below — so ONE picker works from the shell (gateway/Tauri) and from an
// extension (its host bridge) alike. The result of a pick is a `SourceSelection` (a `{tool,args}` read
// source, a `{tool,argsTemplate}` write action, or an `ext:<id>/<widget>` view key) — never a
// dashboard `Cell`; the host maps a selection onto whatever it stores.
//
// The row shapes MIRROR the node's wire records (the same fields `ext.list` / `flows.*` /
// `series.list` / `datasource.list` return). They live here so the package stands alone; the shell's
// `@/lib/*` types re-export / structurally match these (one shape, not two).

/** A read source — ANY granted MCP tool call (re-checked at the host per call). */
export interface Source {
  tool: string;
  args?: Record<string, unknown>;
}

/** A write action — the tool a switch/slider/button calls on interaction. `argsTemplate` carries a
 *  `{{value}}` slot the interaction fills. */
export interface Action {
  tool: string;
  argsTemplate?: Record<string, unknown>;
}

/** A page/widget an extension contributes (mirrors the node's `ExtUi`). */
export interface ExtUi {
  entry: string;
  label: string;
  icon: string;
  scope: string[];
  /** `true` for a frames-in DATA widget (manifest `data = true`) — it keeps the cell's `sources[]`. */
  data?: boolean;
}

/** An installed extension row (the subset the picker needs from `ext.list`). */
export interface ExtRow {
  ext: string;
  enabled: boolean;
  ui?: ExtUi | null;
  widgets?: ExtUi[];
}

/** A flow's summary (from `flows.list`). */
export interface FlowSummary {
  id: string;
  name: string;
}

/** A flow node (the subset the picker reads to enumerate ports). */
export interface FlowNode {
  id: string;
  type: string;
}

/** A full flow (from `flows.get`) — only the fields the picker walks. */
export interface Flow {
  id: string;
  name: string;
  nodes?: FlowNode[];
}

/** A node descriptor (from `flows.nodes`) — the port lists the picker offers as bindings. */
export interface NodeDescriptor {
  type: string;
  inputs?: string[];
  outputs?: string[];
}

/** A registered federation datasource (from `datasource.list`). */
export interface DatasourceRow {
  name: string;
  kind: string;
}

/** The INJECTED read seam. The host implements each over its own transport (the shell delegates to
 *  its `@/lib/*` clients; an extension calls its `bridge.call`). Every function is allowed to reject /
 *  return empty — the loader hook treats a failure as "that group is empty" (honest, capability-scoped
 *  offer), exactly as the shipped `useSourcePicker` does. All are optional: a host that only wants
 *  series passes just `listSeries`; absent loaders yield absent groups. */
export interface SourceLoaders {
  /** Concrete series names (from `series.list`/`series.find`). Drives the Series + Live groups. */
  listSeries?: () => Promise<string[]>;
  /** Installed extensions (from `ext.list`). Drives the Installed-extension + Extension-widget groups. */
  listExtensions?: () => Promise<ExtRow[]>;
  /** Flow summaries the caller may reach (from `flows.list`). */
  listFlows?: () => Promise<FlowSummary[]>;
  /** One flow's full graph (from `flows.get`). Called per summary; a denied flow is skipped. */
  getFlow?: (id: string) => Promise<Flow | null>;
  /** Node descriptors (from `flows.nodes`) — the port lists for the Flows group. */
  listFlowNodes?: () => Promise<NodeDescriptor[]>;
  /** Registered federation datasources (from `datasource.list`). Drives the Datasource dropdown. */
  listDatasources?: () => Promise<DatasourceRow[]>;
}

/** What selecting a picker entry yields — the host maps this onto whatever it persists (a dashboard
 *  cell, a scene bind, a variable query, …). Exactly one of `source`/`action`/`viewKey` is set. */
export interface SourceSelection {
  /** The chosen entry's id (stable, for round-trip seeding). */
  id: string;
  /** A read source `{tool,args}` (series/live/sql/extension/flows-output). */
  source?: Source;
  /** A write action `{tool,argsTemplate}` (flows-input / a write extension tool). */
  action?: Action;
  /** A packaged tile view key `ext:<id>/<widget>` (a finished extension widget). */
  viewKey?: string;
}
