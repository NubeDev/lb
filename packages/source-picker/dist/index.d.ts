import { JSX as JSX_2 } from 'react';

/** A write action — the tool a switch/slider/button calls on interaction. `argsTemplate` carries a
 *  `{{value}}` slot the interaction fills. */
export declare interface Action {
    tool: string;
    argsTemplate?: Record<string, unknown>;
}

/** Assemble the whole picker from loader results. Series/live from `series`; extension + widget from
 *  `extensions`; flows from `flows`+`descriptors`; the SQL entry is always offered (the host's parse
 *  gate + ws wall make it safe regardless of which tables exist). Datasources are the DROPDOWN roster
 *  (`SourceInputs.datasources`), surfaced by the UI separately from these entries. */
export declare function buildSourceEntries(inputs: SourceInputs): SourceEntry[];

/** A registered federation datasource (from `datasource.list`). */
export declare interface DatasourceRow {
    name: string;
    kind: string;
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

/** Live (Zenoh) entries — each series also offers a live `series.watch` stream. */
export declare function liveEntries(seriesNames: string[]): SourceEntry[];

/** Run every loader (deny-tolerant) and fold the results into picker entries. The Flows group composes
 *  `flows.list` (flows the caller may reach) + `flows.nodes` (descriptors) + a `flows.get` per flow; a
 *  flow the caller cannot `flows.get` is silently skipped. */
export declare function loadSourcePicker(loaders: SourceLoaders): Promise<SourcePickerResult>;

/** A node descriptor (from `flows.nodes`) — the port lists the picker offers as bindings. */
export declare interface NodeDescriptor {
    type: string;
    inputs?: string[];
    outputs?: string[];
}

/** Fold a chosen entry into a `SourceSelection` (drop the labelling fields; keep what the host stores). */
export declare function selectionOf(entry: SourceEntry): {
    id: string;
    source?: Source;
    action?: Action;
    viewKey?: string;
};

/** Series entries — each ⇒ `series.read` of that series. */
export declare function seriesEntries(seriesNames: string[]): SourceEntry[];

/** A read source — ANY granted MCP tool call (re-checked at the host per call). */
export declare interface Source {
    tool: string;
    args?: Record<string, unknown>;
}

/** A friendly source entry the picker offers. `group` places it; `source`/`action`/`viewKey` is what
 *  selecting it yields (folded into a `SourceSelection` by the caller). */
export declare interface SourceEntry {
    /** Stable id for the option element + round-trip seeding. */
    id: string;
    /** The grouping origin (the picker's sections). `widget` is a packaged `[[widget]]` tile (a finished
     *  widget the developer shipped — distinct from `extension`, which offers an extension's raw tools). */
    group: "series" | "live" | "extension" | "action" | "sql" | "widget" | "flows";
    /** What the author sees — never a raw tool name. */
    label: string;
    /** For a `widget` entry: the icon name the tile declared (lucide id). */
    icon?: string;
    /** For a `widget` entry: the `ext:<id>/<widget>` view key the tile resolves to. */
    viewKey?: string;
    /** The resolved read source `{tool,args}` (read/scripted views + a control's optional self-read). */
    source?: Source;
    /** The resolved write action (control views) — `argsTemplate` gets a `{{value}}` slot filled later. */
    action?: Action;
    /** True if the entry's tool writes (drives the Action group + write-capable views). */
    writes: boolean;
}

/** Inputs to `buildSourceEntries` — the loader results, each optional (absent → that group is absent). */
export declare interface SourceInputs {
    series?: string[];
    extensions?: ExtRow[];
    flows?: Flow[];
    descriptors?: NodeDescriptor[];
    datasources?: DatasourceRow[];
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
