import { JSX as JSX_2 } from 'react';
import { ReactNode } from 'react';

/** A write action — the tool a switch/slider/button calls on interaction. `argsTemplate` carries a
 *  `{{value}}` slot the interaction fills. */
export declare interface Action {
    tool: string;
    argsTemplate?: Record<string, unknown>;
}

/** The builder's group list — the read groups plus the `action` (write control) group, ordered as the
 *  widget builder shows them (action before widget). A host authoring controls uses this. */
export declare const BUILDER_SOURCE_GROUPS: SourceGroup[];

/** Assemble the whole picker from loader results. Series/live from `series`; extension + widget from
 *  `extensions`; flows from `flows`+`descriptors`; the SQL entry is always offered (the host's parse
 *  gate + ws wall make it safe regardless of which tables exist). Datasources are the DROPDOWN roster
 *  (`SourceInputs.datasources`), surfaced by the UI separately from these entries. */
export declare function buildSourceEntries(inputs: SourceInputs): SourceEntry[];

/** The canonical section registry. A host renders whichever of these its loaders cover; ids stay
 *  opaque (rule 10 — no core branch on a host's "known subsystem list"). */
export declare const CATALOG_SECTION_SPECS: CatalogSectionSpec[];

/** A teaching empty state — used by per-kind row renderers when the section is ready but holds zero
 *  rows (e.g. "No external datasources registered."). */
export declare function CatalogEmpty({ children }: {
    children: ReactNode;
}): JSX_2.Element;

/** What a click in the explorer yields — a tagged row the HOST maps onto its snippet/bind. Each kind
 *  carries ONLY the fields a host needs to form that mapping; the package owns no host semantics
 *  (rule 10). The host's `onSelect` is the one place "what this pick MEANS" is decided. */
export declare type CatalogEntry = {
    kind: "datasource";
    id: string;
    name: string;
    rowKind: string;
    endpoint?: string;
} | {
    kind: "table";
    id: string;
    table: string;
} | {
    kind: "column";
    id: string;
    table: string;
    column: string;
} | {
    kind: "series";
    id: string;
    name: string;
} | {
    kind: "channel";
    id: string;
    name: string;
} | {
    kind: "insight";
    id: string;
    title: string;
    severity?: string;
    status?: string;
} | {
    kind: "inbox";
    id: string;
    channel: string;
};

/** The system-catalog explorer panel. */
export declare function CatalogExplorer({ sections, onSelect, sectionSpecs, className, }: CatalogExplorerProps): JSX_2.Element;

export declare interface CatalogExplorerProps {
    /** The per-section state from `useCatalog`. Sections absent here (the host wired no loader) are
     *  skipped even if `sections` lists them — absent loader ⇒ absent section. */
    sections: CatalogSections;
    /** Called with the picked `CatalogEntry` whenever a row is clicked. The host maps the entry onto
     *  its own snippet/bind (a Rhai `source("name")`, a SQL table name, a dashboard cell source). */
    onSelect: (entry: CatalogEntry) => void;
    /** Which sections to render + their labels/hints, in display order. Defaults to the canonical
     *  `CATALOG_SECTION_SPECS`. A host that wants a subset (e.g. just `datasources` + `series`) passes
     *  its own filtered list. */
    sectionSpecs?: CatalogSectionSpec[];
    /** Extra className on the root. */
    className?: string;
}

/** A table → column tree with click-to-pick. Tolerates an empty schema (the parent shows the
 *  teaching-empty/deny; this renders nothing for `tables: []`). */
export declare function CatalogSchemaTree({ schema, onSelect }: CatalogSchemaTreeProps): JSX_2.Element;

export declare interface CatalogSchemaTreeProps {
    schema: Schema;
    /** Called when a table header (no `column`) or a column row is clicked. */
    onSelect: (entry: CatalogEntry) => void;
}

/** Render a section's header/hint + its honest state: loading skeleton, "Not permitted." deny, the
 *  teaching empty (when `ready` returns null/[]), or the ready body. */
export declare function CatalogSection<T>({ spec, state, children }: CatalogSectionProps<T>): JSX_2.Element;

/** The schema of `CatalogSections.data` per section kind. The explorer kinds carry row arrays (or
 *  `Schema` for the local-tables section, which the tree renderer walks); the picker-only kinds
 *  (`extensions`/`rules`/`flowSummaries`/`flowDescriptors`) carry the row shapes `loadSourcePicker`
 *  composes from. */
export declare interface CatalogSectionData {
    datasources: DatasourceRow[];
    schema: Schema;
    series: string[];
    channels: ChannelRow[];
    insights: InsightRow[];
    inbox: InboxRow[];
    extensions: ExtRow[];
    rules: RuleSummary[];
    flowSummaries: FlowSummary[];
    flowDescriptors: NodeDescriptor[];
}

/** The catalog's section vocabulary. Each kind is 1:1 with a single `SourceLoaders` read. Adding a
 *  section = adding a kind here + a row type + a loader entry on `SourceLoaders`. The renderer is
 *  kind-agnostic (it renders a `CatalogSectionSpec`'s label/hint + the section's `SectionState`),
 *  so a new kind needs no renderer change.
 *
 *  NOTE: this is the FULL vocabulary the catalog CAN cover (so `loadSourcePicker` projects every
 *  loader it needs off the same per-section state). `CATALOG_SECTION_SPECS` below is the SUBSET the
 *  EXPLORER skin renders today — a host composes which sections its surface shows. `extensions`,
 *  `rules`, `flowSummaries`, `flowDescriptors` are picker-only projections today (no explorer
 *  section) but share the orchestration. */
export declare type CatalogSectionKind = "datasources" | "schema" | "series" | "channels" | "insights" | "inbox" | "extensions" | "rules" | "flowSummaries" | "flowDescriptors";

export declare interface CatalogSectionProps<T> {
    spec: CatalogSectionSpec;
    state: SectionState<T>;
    /** The ready-body renderer — receives the section's data and returns the row tree. The explorer
     *  composes this per kind (datasource rows / the schema table tree / channel rows / …). */
    children: (data: T) => ReactNode;
}

/** The catalog's per-section honest state. A section is `undefined` when the host supplied no
 *  loader for it (absent ⇒ absent section); `{status:"loading"}` while in flight; `{status:"ready"}`
 *  on success; `{status:"denied"}` on throw (capability wall — never a fake list). */
export declare type CatalogSections = {
    [K in CatalogSectionKind]?: SectionState<CatalogSectionData[K]>;
};

/** A section's declarative descriptor — its kind (loader-keyed), its human label, and a one-line
 *  hint. Exported as `CATALOG_SECTION_SPECS` (the canonical list); a host composes its surface by
 *  which loaders it wires (absent loader ⇒ absent section). */
export declare interface CatalogSectionSpec {
    kind: CatalogSectionKind;
    label: string;
    hint: string;
}

/** Channel rows → catalog entries. */
export declare function channelEntries(rows: ChannelRow[]): CatalogEntry[];

/** A registered channel row (the subset of `channel.list` the catalog needs — id only; the registry
 *  record carries more, the package keeps the seam minimal). */
export declare interface ChannelRow {
    id: string;
}

/** Datasource rows → catalog entries. The id is the name (stable round-trip key). */
export declare function datasourceEntries(rows: DatasourceRow[]): CatalogEntry[];

/** A registered federation datasource (from `datasource.list`). */
export declare interface DatasourceRow {
    name: string;
    kind: string;
    /** Optional endpoint label (mirrors `datasource.list`'s `endpoint`). The catalog row renders it as
     *  a `kind · endpoint` sub-label; absent ⇒ just `kind`. */
    endpoint?: string;
}

/** Installed-extension TOOL entries — split an extension's `ui`/`widgets[]` scope tools into READ
 *  sources and WRITE actions by name heuristic. (A tile's finished-widget entry is `extWidgetEntries`.) */
export declare function extensionEntries(rows: ExtRow[]): SourceEntry[];

/** An installed extension row (the subset the picker needs from `ext.list`). */
export declare interface ExtRow {
    ext: string;
    enabled: boolean;
    ui?: ExtUi | null;
    widgets?: ExtUi[];
}

/** A page/widget an extension contributes (mirrors the node's `ExtUi`). */
export declare interface ExtUi {
    entry: string;
    label: string;
    icon: string;
    scope: string[];
    /** `true` for a frames-in DATA widget (manifest `data = true`) — it keeps the cell's `sources[]`. */
    data?: boolean;
}

/** Packaged-tile entries — ONE per `row.widgets[]` `[[widget]]`. Selecting it yields a
 *  `view: ext:<id>/<widget>` (the tile owns its data via `scope ∩ grant`). A disabled ext contributes
 *  none. The `viewKey` uses the SAME `widgetIdOf` slug the renderer parses. */
export declare function extWidgetEntries(rows: ExtRow[]): SourceEntry[];

/** A full flow (from `flows.get`) — only the fields the picker walks. */
export declare interface Flow {
    id: string;
    name: string;
    nodes?: FlowNode[];
}

/** A flow node (the subset the picker reads to enumerate ports). */
export declare interface FlowNode {
    id: string;
    type: string;
}

/** Flows entries — one per (flow, node, INPUT/OUTPUT port). An INPUT port → a write Action
 *  (`flows.inject`, a control drives the node's retained input); an OUTPUT port → a read Source
 *  (`flows.node_state`, extract this node's port). A node whose descriptor is missing contributes no
 *  ports (honest empty, never a guess). The author sees `flow › node › port (input|output)`. */
export declare function flowsEntries(flows: Flow[], descriptors: NodeDescriptor[]): SourceEntry[];

/** A flow's summary (from `flows.list`). */
export declare interface FlowSummary {
    id: string;
    name: string;
}

/** Inbox rows → catalog entries. */
export declare function inboxEntries(rows: InboxRow[]): CatalogEntry[];

/** An inbox item summary row (the subset of `inbox.list`'s `Item` the catalog renders). */
export declare interface InboxRow {
    id: string;
    channel: string;
}

/** Insight rows → catalog entries. */
export declare function insightEntries(rows: InsightRow[]): CatalogEntry[];

/** An insight summary row (the subset of `insight.list`'s `items[]` the catalog renders). Severity
 *  + status are optional so a host that only has `id`/`title` still renders. */
export declare interface InsightRow {
    id: string;
    title: string;
    severity?: string;
    status?: string;
}

/** Live (Zenoh) entries — each series also offers a live `series.watch` stream. */
export declare function liveEntries(seriesNames: string[]): SourceEntry[];

/** Run every loader the host wired (deny-tolerant per section). Each present loader resolves to
 *  `ready`/`denied` independently; absent loaders yield an absent (undefined) section. The
 *  orchestration is the single source of truth — the picker's deny→empty collapse and the
 *  explorer's visible tri-state both project off the record this returns.
 *
 *  `publish` (optional) is invoked once per section as it resolves, with the cumulative
 *  `CatalogSections` record — so a caller (the `useCatalog` hook) can surface each section's state
 *  the moment it lands instead of waiting for every loader. Late calls after the caller is
 *  unmounted/cancelled are the caller's concern (it passes a `publish` that no-ops on cancel). */
export declare function loadCatalog(loaders: SourceLoaders, publish?: (merge: (current: CatalogSections) => CatalogSections) => void): Promise<CatalogSections>;

/** Run every loader (deny-tolerant; absent loader ⇒ absent input) and fold the results into picker
 *  entries. The Flows group composes `flows.list` + `flows.nodes` + a per-flow `flows.get` — the
 *  catalog exposes the first two as `flowSummaries`/`flowDescriptors`; `getFlow` is per-flow so it
 *  stays picker-side (the catalog is a per-loader record, not a per-item join). */
export declare function loadSourcePicker(loaders: SourceLoaders): Promise<SourcePickerResult>;

/** A node descriptor (from `flows.nodes`) — the port lists the picker offers as bindings. */
export declare interface NodeDescriptor {
    type: string;
    inputs?: string[];
    outputs?: string[];
}

/** The declared type of a rule param — steers the host's input control + value coercion (mirrors the
 *  node's `ParamKind`). Absent → `"text"`. */
export declare type ParamKind = "text" | "number" | "date" | "enum";

/** One `<optgroup>` for a source group, empty-tolerant (no section when it has no entries). Exported so a
 *  host that renders its own `<select>` (shadcn `Select`, a `FIELD`-classed native select) still uses the
 *  ONE grouping/labelling implementation — the `<optgroup>` carries no styling, so it drops into any select. */
export declare function PickerGroup({ entries, group, label, }: {
    entries: SourceEntry[];
    group: SourceEntry["group"];
    label: string;
}): JSX_2.Element | null;

/** The read/source groups, in display order, with their section labels. `action` is omitted (write
 *  controls are a separate authoring intent); a host that wants them passes its own list (see
 *  `BUILDER_SOURCE_GROUPS`). Exported so every consumer renders ONE canonical label set. */
export declare const READ_SOURCE_GROUPS: SourceGroup[];

/** A rule's declared parameter (mirrors the node's `RuleParam`) — a name, an optional human label, and
 *  its type. A host renders one input per param around the picker and fills the rule's `args.params`.
 *  `kind`/`required`/`options` are optional so a legacy `{name,label}` rule is unaffected. */
export declare interface RuleParam {
    name: string;
    label?: string;
    kind?: ParamKind;
    required?: boolean;
    /** Allowed values for an `enum` param (ignored otherwise). */
    options?: string[];
}

/** Rules entries — one per saved rule. Each ⇒ a read `rules.run {rule_id}` source: the rule fetches
 *  from the gated sources, computes over the rows in the cage (the data-stdlib: time/stats/`Frame`),
 *  and RETURNS records the panel draws (rules-as-source-scope). A rule is the most general query — the
 *  picker offers it as one opaque tool source, re-gated at the host per call (`mcp:rules.run:call`);
 *  whether its output is chart-shaped is the rule author's concern, an honest failure if not. */
export declare function rulesEntries(rules: RuleSummary[]): SourceEntry[];

/** A saved rule's summary (the subset of `rules.list` the picker needs) — a rule is a read source
 *  (`rules.run {rule_id}` → records), so it mirrors `FlowSummary`. `params` (optional) are the rule's
 *  declared inputs; the picker carries them onto the entry so a host can offer a params form. */
export declare interface RuleSummary {
    id: string;
    name: string;
    params?: RuleParam[];
}

/** The workspace's local-store schema (every table + its columns) — the result of `readSchema`. */
export declare interface Schema {
    tables: SchemaTable[];
}

/** One column of a local-store table as `store.schema` reports it (mirrors the shell's `SchemaColumn`
 *  shape, homed here so the package stands alone — system-catalog scope). */
export declare interface SchemaColumn {
    name: string;
    type: string;
}

/** Schema → (table, column) entries — the columns of every table, flattened. The explorer's tree
 *  groups these under their table; the package exposes them flat so a host that wants a flat
 *  column picker can also consume them. */
export declare function schemaColumnEntries(schema: Schema): CatalogEntry[];

/** One local-store table + its columns (the `store.schema` row shape). */
export declare interface SchemaTable {
    name: string;
    columns: SchemaColumn[];
}

/** Schema → table entries (one per table). Columns are addressed by the `column` kind via
 *  `schemaColumnEntries` (the explorer's table→column tree opens a table, then lists its columns). */
export declare function schemaTableEntries(schema: Schema): CatalogEntry[];

/** A section's load state — never a fake "ready with empty data" when the read was denied. This is
 *  the contract the EXPLORER skin surfaces visibly (loading skeleton / "Not permitted." / ready) and
 *  the COMBOBOX collapses into an empty group via projection. Moved in from the rules panel's
 *  `useDataExplorer` (system-catalog scope). */
export declare type SectionState<T> = {
    status: "loading";
} | {
    status: "ready";
    data: T;
} | {
    status: "denied";
    error: string;
};

/** Fold a chosen entry into a `SourceSelection` (drop the labelling fields; keep what the host stores). */
export declare function selectionOf(entry: SourceEntry): {
    id: string;
    source?: Source;
    action?: Action;
    viewKey?: string;
};

/** Series names → catalog entries (one per series). */
export declare function seriesCatalogEntries(names: string[]): CatalogEntry[];

/** Series entries — each ⇒ `series.read` of that series. */
export declare function seriesEntries(seriesNames: string[]): SourceEntry[];

/** A read source — ANY granted MCP tool call (re-checked at the host per call). */
export declare interface Source {
    tool: string;
    args?: Record<string, unknown>;
}

export declare function SourceCombobox({ entries, value, onSelect, onSelectEntry, loading, groups, "aria-label": ariaLabel, className, placeholder, autoFocus, }: SourceComboboxProps): JSX_2.Element;

export declare interface SourceComboboxProps {
    /** The assembled entries (from `useSourcePicker`). */
    entries: SourceEntry[];
    /** The currently-selected entry id (controlled) — "" for none. */
    value?: string;
    /** Called with the chosen entry's selection (or null when cleared). */
    onSelect: (selection: SourceSelection | null) => void;
    /** Also called with the RAW entry (or null) — for a host that keys on `entry.id` (e.g. edit-mode
     *  seeding, or a tool shared across entries like `rules.run`) where the folded selection loses the id.
     *  Optional; `onSelect` fires regardless. */
    onSelectEntry?: (entry: SourceEntry | null) => void;
    /** True while the entries load. */
    loading?: boolean;
    /** Which groups show + their order/labels (default: the read groups). */
    groups?: SourceGroup[];
    /** Accessible label (default "source"). */
    "aria-label"?: string;
    /** Extra className on the root. */
    className?: string;
    /** Placeholder for the search input. */
    placeholder?: string;
    /** Autofocus the search box on mount (Data Studio focuses it so type-to-search is the first action). */
    autoFocus?: boolean;
}

/** A friendly source entry the picker offers. `group` places it; `source`/`action`/`viewKey` is what
 *  selecting it yields (folded into a `SourceSelection` by the caller). */
export declare interface SourceEntry {
    /** Stable id for the option element + round-trip seeding. */
    id: string;
    /** The grouping origin (the picker's sections). `widget` is a packaged `[[widget]]` tile (a finished
     *  widget the developer shipped — distinct from `extension`, which offers an extension's raw tools). */
    group: "series" | "live" | "extension" | "action" | "sql" | "widget" | "flows" | "rules";
    /** What the author sees — never a raw tool name. */
    label: string;
    /** For a `widget` entry: the icon name the tile declared (lucide id). */
    icon?: string;
    /** For a `widget` entry: the `ext:<id>/<widget>` view key the tile resolves to. */
    viewKey?: string;
    /** For a `widget` entry: `true` if the tile is a frames-in DATA view (its manifest set `data = true`).
     *  A data widget KEEPS the cell's `sources[]` (the shell resolves them to `ctx.data`) and shows the
     *  Query + Field tabs; a non-data widget owns its own data and clears targets when picked. */
    data?: boolean;
    /** The resolved read source `{tool,args}` (read/scripted views + a control's optional self-read). */
    source?: Source;
    /** The resolved write action (control views) — `argsTemplate` gets a `{{value}}` slot filled later. */
    action?: Action;
    /** True if the entry's tool writes (drives the Action group + write-capable views). */
    writes: boolean;
    /** For a `rules` entry: the rule's declared params, so a host can render a params form around the
     *  picker and fill the `rules.run` `args.params` (a rule with no params has none/empty). */
    params?: RuleParam[];
}

/** One entry in a picker's group list: which source `group` to render and its section label. */
export declare type SourceGroup = {
    group: SourceEntry["group"];
    label: string;
};

/** Inputs to `buildSourceEntries` — the loader results, each optional (absent → that group is absent). */
export declare interface SourceInputs {
    series?: string[];
    extensions?: ExtRow[];
    flows?: Flow[];
    descriptors?: NodeDescriptor[];
    datasources?: DatasourceRow[];
    rules?: RuleSummary[];
}

/** The INJECTED read seam. The host implements each over its own transport (the shell delegates to
 *  its `@/lib/*` clients; an extension calls its `bridge.call`). Every function is allowed to reject /
 *  return empty — the loader hook treats a failure as "that group is empty" (honest, capability-scoped
 *  offer), exactly as the shipped `useSourcePicker` does. All are optional: a host that only wants
 *  series passes just `listSeries`; absent loaders yield absent groups. */
export declare interface SourceLoaders {
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

export declare function SourcePicker({ entries, value, onSelect, loading, groups, "aria-label": ariaLabel, className, }: SourcePickerProps): JSX_2.Element;

export declare interface SourcePickerData {
    entries: SourceEntry[];
    /** The installed extensions (also handed to a cell renderer for `ext:<id>/<widget>` tiles). */
    installed: ExtRow[];
    loading: boolean;
}

export declare interface SourcePickerProps {
    /** The assembled entries (from `useSourcePicker`). */
    entries: SourceEntry[];
    /** The currently-selected entry id (controlled) — "" for none. */
    value?: string;
    /** Called with the chosen entry's selection (or null when cleared to "— pick —"). */
    onSelect: (selection: SourceSelection | null) => void;
    /** True while the entries load — shows a loading placeholder. */
    loading?: boolean;
    /** Override which groups show + their order/labels (default: the read groups above). */
    groups?: {
        group: SourceEntry["group"];
        label: string;
    }[];
    /** Accessible label for the select (default "source"). */
    "aria-label"?: string;
    /** Extra className on the root <label> (host layout). */
    className?: string;
}

/** The assembled picker data (sans loading flag — the caller owns that). */
export declare interface SourcePickerResult {
    entries: SourceEntry[];
    installed: ExtRow[];
}

/** What selecting a picker entry yields — the host maps this onto whatever it persists (a dashboard
 *  cell, a scene bind, a variable query, …). Exactly one of `source`/`action`/`viewKey` is set. */
export declare interface SourceSelection {
    /** The chosen entry's id (stable, for round-trip seeding). */
    id: string;
    /** A read source `{tool,args}` (series/live/sql/extension/flows-output). */
    source?: Source;
    /** A write action `{tool,argsTemplate}` (flows-input / a write extension tool). */
    action?: Action;
    /** A packaged tile view key `ext:<id>/<widget>` (a finished extension widget). */
    viewKey?: string;
}

/** The id of the "SQL query" entry — the visual SQL builder + raw-SQL source over `store.query`. */
export declare const SQL_SOURCE_ID = "sql:query";

/** The single "SQL query" picker entry. Its `source.tool` is `store.query` so a host's tool set
 *  includes it (the bridge's leash); the concrete `sql` is filled by the host's SQL editor. */
export declare function sqlSourceEntry(): SourceEntry;

/** Load + surface the catalog. `loaders` is the host's read seam; `ws` keys the re-load (the
 *  workspace switch). The effect keys on `ws` ONLY and reads `loaders` through a ref kept current
 *  every render — so a fresh loaders literal per render does NOT loop (same discipline as
 *  `useSourcePicker`). A host that swaps to a genuinely different transport should also change `ws`. */
export declare function useCatalog(loaders: SourceLoaders, ws: string): CatalogSections;

/** Load + assemble the picker. `loaders` is the host's read seam; `ws` keys the re-load (the workspace
 *  switch). The effect keys on `ws` ONLY and reads `loaders` through a ref kept current every render —
 *  so an UNMEMOIZED `loaders` object (a fresh literal per render, the easy host mistake) does NOT loop.
 *  A host that swaps to a genuinely different transport should also change `ws` (or remount). */
export declare function useSourcePicker(loaders: SourceLoaders, ws: string): SourcePickerData;

/** Derive a widget id from a tile — the label slug, lowercased, non-alnum → `-`. The renderer parses
 *  the same slug from the `ext:<id>/<widget>` key, so picker and renderer agree (one slug function).
 *  Exported so a host renderer can reuse it instead of forking a second slugger. */
export declare function widgetIdOf(w: {
    label: string;
}): string;

export { }
