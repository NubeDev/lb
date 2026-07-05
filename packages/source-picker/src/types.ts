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

/** The declared type of a rule param — steers the host's input control + value coercion (mirrors the
 *  node's `ParamKind`). Absent → `"text"`. */
export type ParamKind = "text" | "number" | "date" | "enum";

/** A rule's declared parameter (mirrors the node's `RuleParam`) — a name, an optional human label, and
 *  its type. A host renders one input per param around the picker and fills the rule's `args.params`.
 *  `kind`/`required`/`options` are optional so a legacy `{name,label}` rule is unaffected. */
export interface RuleParam {
  name: string;
  label?: string;
  kind?: ParamKind;
  required?: boolean;
  /** Allowed values for an `enum` param (ignored otherwise). */
  options?: string[];
}

/** A saved rule's summary (the subset of `rules.list` the picker needs) — a rule is a read source
 *  (`rules.run {rule_id}` → records), so it mirrors `FlowSummary`. `params` (optional) are the rule's
 *  declared inputs; the picker carries them onto the entry so a host can offer a params form. */
export interface RuleSummary {
  id: string;
  name: string;
  params?: RuleParam[];
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
  /** Optional endpoint label (mirrors `datasource.list`'s `endpoint`). The catalog row renders it as
   *  a `kind · endpoint` sub-label; absent ⇒ just `kind`. */
  endpoint?: string;
}

/** One column of a local-store table as `store.schema` reports it (mirrors the shell's `SchemaColumn`
 *  shape, homed here so the package stands alone — system-catalog scope). */
export interface SchemaColumn {
  name: string;
  type: string;
}

/** One local-store table + its columns (the `store.schema` row shape). */
export interface SchemaTable {
  name: string;
  columns: SchemaColumn[];
}

/** The workspace's local-store schema (every table + its columns) — the result of `readSchema`. */
export interface Schema {
  tables: SchemaTable[];
}

/** A registered channel row (the subset of `channel.list` the catalog needs — id only; the registry
 *  record carries more, the package keeps the seam minimal). */
export interface ChannelRow {
  id: string;
}

/** An insight summary row (the subset of `insight.list`'s `items[]` the catalog renders). Severity
 *  + status are optional so a host that only has `id`/`title` still renders. */
export interface InsightRow {
  id: string;
  title: string;
  severity?: string;
  status?: string;
}

/** An inbox item summary row (the subset of `inbox.list`'s `Item` the catalog renders). */
export interface InboxRow {
  id: string;
  channel: string;
}

/** A section's load state — never a fake "ready with empty data" when the read was denied. This is
 *  the contract the EXPLORER skin surfaces visibly (loading skeleton / "Not permitted." / ready) and
 *  the COMBOBOX collapses into an empty group via projection. Moved in from the rules panel's
 *  `useDataExplorer` (system-catalog scope).
 *
 *  `idle` is the lazy-load contract: the section is collapsed and its loader has NOT fired yet. The
 *  loader fires the first time a user expands the section (the explorer's `onOpen`), then transitions
 *  to `loading` → `ready`/`denied`. Subsequent collapse/re-expand keeps the cached data (no refire). */
export type SectionState<T> =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "ready"; data: T }
  | { status: "denied"; error: string };

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
  /** Saved rules the caller may run (from `rules.list`). Drives the Rules group — each ⇒ a `rules.run`
   *  read source (the rule fetches + computes in the cage and returns records the panel draws). */
  listRules?: () => Promise<RuleSummary[]>;
  /** The workspace's local-store schema (from `store.schema`). Drives the explorer's Local-tables
   *  section (table → column tree). Absent ⇒ the section is absent (a host that only wants the
   *  picker groups skips it). */
  readSchema?: () => Promise<Schema>;
  /** Registered channels (from `channel.list`). Drives the explorer's Channels section. */
  listChannels?: () => Promise<ChannelRow[]>;
  /** Insights (from `insight.list`). Drives the explorer's Insights section. The host may pre-filter
   *  (status/severity) in its loader closure — the package just enumerates what it returns. */
  listInsights?: () => Promise<InsightRow[]>;
  /** Inbox items (from `inbox.list`). `inbox.list` is per-channel, so the host fixes the channel in
   *  its loader closure; the package calls it with no args. */
  listInbox?: () => Promise<InboxRow[]>;
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
