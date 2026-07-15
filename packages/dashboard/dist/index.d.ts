import { ComponentType } from 'react';
import { JSX as JSX_2 } from 'react';

/** A control's write action — the tool a switch/slider/button calls on interaction. */
export declare interface Action {
    tool: string;
    argsTemplate?: Record<string, unknown>;
}

/** A cell's data binding: an explicit series name, OR a tag-facet query. */
export declare type Binding = {
    series: string;
} | {
    find: {
        tags: string[];
    };
};

/** Narrow a binding to its explicit series name, if it has one. */
export declare function bindingSeries(binding: Binding): string | null;

/** The tag strings (`key:value`) of a tag-facet binding, or `[]` for a series binding. */
export declare function bindingTags(binding: Binding): string[];

/** Resolve a view to its canonical panel-type id (so `view:"chart"` and `view:"timeseries"`
 *  dispatch to the one registered renderer). Non-aliased views pass through. */
export declare function canonicalView(view: View | string): View;

/** One grid cell — react-grid-layout geometry + the widget it hosts + its binding/source +
 *  options. v1 cells carry `widget_type` + `binding`; v2 add `v:2`, `view`, `source`, `action`;
 *  v3 fields are all additive/optional. A renderer reads `view` when present, else falls back to
 *  `widget_type` (see {@link cellView}). */
export declare interface Cell {
    /** react-grid-layout item key (stable per cell). */
    i: string;
    x: number;
    y: number;
    w: number;
    h: number;
    /** Contract version. Absent/0/1 = a v1 series cell; 2 = a v2 tool-bound cell. */
    v?: number;
    widget_type: WidgetType;
    /** A human title for the cell; the header renders it, falling back to a derived label. */
    title?: string;
    /** v2 render vocabulary. Empty on a v1 cell. */
    view?: View;
    /** v1 binding (kept for v1 compatibility). */
    binding: Binding;
    /** v2 source: the `{ tool, args }` the cell reads/streams. */
    source?: Source;
    /** v2 action: a control's write `{ tool, argsTemplate }`. */
    action?: Action;
    /** Widget-type-specific options (unit label, thresholds, range, inline template code…). */
    options?: Record<string, unknown>;
    description?: string;
    /** v3 targets — supersedes the single `source` (read through {@link cellSources}). */
    sources?: Target[];
    transformations?: Transformation[];
    queryOptions?: QueryOptions;
    /** Per-field option defaults + overrides. CARRIED as data in v0.1 — the apply/format bridge
     *  is NOT in this package; a consumer's renderers interpret it. */
    fieldConfig?: FieldConfig;
    pluginVersion?: string;
    /** Grafana's panel `transparent`: drop the frame chrome and sit directly on the board. */
    transparent?: boolean;
    /** Grafana panel `links` — a titled URL list shown in the panel header. */
    links?: DataLink[];
    panelRef?: string;
    panelVars?: Record<string, unknown>;
    /** Set by host hydration when a ref can't resolve — renderers show an honest placeholder. */
    panelMissing?: boolean;
}

/** A cell's effective field-config, defaulted to empty. */
export declare function cellFieldConfig(cell: Cell): FieldConfig;

/** A cell's header label: the author `title` when set, else a derived fallback — the source
 *  tool, an action tool, or the view. Never the empty string. */
export declare function cellLabel(cell: Cell): string;

/** A cell's primary (first non-hidden) target — what a single-source view reads. */
export declare function cellPrimaryTarget(cell: Cell): Target | undefined;

/** A cell's targets, v3 — `sources[]` when present, else the v2 single `source` as `[A]`, else
 *  `[]`. The ONE adapter that lets a renderer treat a v2 cell as a v3 one-target cell. */
export declare function cellSources(cell: Cell): Target[];

/** Resolve a cell's effective render view — `view` (v2) when present, else `widget_type` (v1) —
 *  CANONICALIZED through the alias map. A cell with NEITHER (malformed / half-authored) defaults
 *  to `timeseries`; a real-but-unknown view still reaches the registry's honest unknown state. */
export declare function cellView(cell: Cell): View;

/** Build a registry, optionally seeded from a `{ view: renderer }` map. */
export declare function createRegistry<S = unknown>(initial?: Record<string, WidgetRenderer<S>>): WidgetRegistry<S>;

/** A full dashboard record (layout + sharing metadata). `variables` is OPAQUE here — the
 *  variables machinery (definitions, URL selection, interpolation) is the consumer's. */
export declare interface Dashboard {
    id: string;
    title: string;
    description?: string;
    icon?: string;
    color?: string;
    toolbar?: Toolbar;
    timezone?: string;
    owner: string;
    visibility: Visibility;
    cells: Cell[];
    /** Variable definitions — opaque to the package (see the module comment). */
    variables?: unknown[];
    schemaVersion?: number;
    updated_ts: number;
    deleted?: boolean;
}

export declare function DashboardGrid<S = unknown>({ cells, editable, registry, range, scope, refreshKey, onLayout, onRemove, onDuplicate, onToggleRow, onRenameRow, onEditPanel, onExportCell, stackBelow, }: DashboardGridProps<S>): JSX_2.Element;

export declare interface DashboardGridProps<S = unknown> {
    cells: Cell[];
    editable: boolean;
    /** The consumer's view → renderer map. An unregistered view renders the honest placeholder. */
    registry: WidgetRegistry<S>;
    /** The dashboard's active time window, passed through to every renderer. */
    range?: TimeRange;
    /** The opaque consumer scope (variables etc.), passed through to every renderer. */
    scope?: S;
    /** Auto-refresh tick, passed through to every renderer. */
    refreshKey?: number;
    /** Called with the new cell geometry on a drag/resize stop (the persistence seam). */
    onLayout: (cells: Cell[]) => void;
    /** Remove a cell. Omitted ⇒ no remove affordance. */
    onRemove?: (i: string) => void;
    /** Append a copy of a cell. Omitted ⇒ no duplicate affordance. */
    onDuplicate?: (i: string) => void;
    /** Toggle a row cell's `options.collapsed` (panel-rows). Omitted ⇒ rows are non-collapsible. */
    onToggleRow?: (i: string) => void;
    /** Rename a row cell inline (panel-rows). Omitted ⇒ read-only row title. */
    onRenameRow?: (i: string, title: string) => void;
    /** Edit this panel (the consumer navigates to its editor). Omitted ⇒ no button. */
    onEditPanel?: (i: string) => void;
    /** Export this single cell. Available to viewers too — exporting a definition doesn't widen
     *  data access. Omitted ⇒ no button. */
    onExportCell?: (i: string) => void;
    /** Below this measured width (px) the board renders as the read-only mobile stack. Default
     *  768 ("below md"); pass 0 to always render the grid. */
    stackBelow?: number;
}

export declare function DashboardStack<S = unknown>({ cells, registry, range, scope, refreshKey, }: DashboardStackProps<S>): JSX_2.Element;

export declare interface DashboardStackProps<S = unknown> {
    cells: Cell[];
    registry: WidgetRegistry<S>;
    range?: TimeRange;
    scope?: S;
    refreshKey?: number;
}

/** The cheap roster row a `list` returns (no cell bodies). */
export declare interface DashboardSummary {
    id: string;
    title: string;
    icon?: string;
    color?: string;
    visibility: Visibility;
    updated_ts: number;
}

/** A field data link (Grafana's `DataLink`) — a titled URL shown on a value's context menu. `url` may
 *  carry `${__value.text}`/`${__field.name}` style interpolation (rendered by the view layer); the
 *  editor authors the title + url + open-in-new-tab flag verbatim so import stays a copy. */
export declare interface DataLink {
    title: string;
    url: string;
    targetBlank?: boolean;
}

/** A datasource reference. `uid` names a registered datasource for federation; absent = native. */
export declare interface DataSourceRef {
    type: "surreal" | "series" | "federation" | string;
    uid?: string;
}

/** A fresh, empty field-config (defaults only). */
export declare function emptyFieldConfig(): FieldConfig;

/** The wildcard key catching every `ext:<id>/<widget>` view a shell mounts via federation. */
export declare const EXT_WILDCARD = "ext:*";

/** The deterministic width used before the container has been measured (and in jsdom tests). */
export declare const FALLBACK_WIDTH = 1200;

export declare interface FieldColor {
    mode: FieldColorModeId;
    fixedColor?: string;
    seriesBy?: "last" | "min" | "max";
}

/** Grafana field-color mode ids — the full 13.2 set (`grafana-data/src/types/fieldColor.ts`,
 *  VERBATIM including the mixed casing: `continuous-BlYlRd` but `continuous-blues`). The
 *  `continuous-*` modes render through the one ramp table (`fieldconfig/ramps.ts`); `thresholds`/
 *  `fixed`/`palette-classic` render as before; the rest map to the accent token until their phase. */
export declare type FieldColorModeId = "thresholds" | "fixed" | "palette-classic" | "palette-classic-by-name" | "continuous-GrYlRd" | "continuous-RdYlGr" | "continuous-BlYlRd" | "continuous-YlRd" | "continuous-BlPu" | "continuous-YlBl" | "continuous-blues" | "continuous-reds" | "continuous-greens" | "continuous-purples" | "continuous-viridis" | "continuous-magma" | "continuous-plasma" | "continuous-inferno" | "continuous-cividis" | "shades";

/** The whole field-config: shared defaults + per-field overrides. */
export declare interface FieldConfig {
    defaults: FieldOptions;
    overrides?: FieldOverride[];
}

/** The per-field option set — Grafana's `FieldConfig` defaults. The `custom` bag holds per-view draw
 *  fields (lineWidth/fillOpacity/drawStyle/axis…), owned by the chart-types layer. */
export declare interface FieldOptions {
    displayName?: string;
    description?: string;
    /** PRESENTATION (widget-kit scope): omit this field from a rendered surface (a table column / a form
     *  field). This is PRESENTATION, NOT SECURITY — a hidden field was still returned by the tool and
     *  crossed the bridge under the VIEWER'S grant; hiding removes it from the surface, it does NOT gate
     *  access. Anything truly secret must be DENIED server-side (a denied source is denied whether or not a
     *  field is hidden); secrets are never merely hidden. Additive (`serde(default)` on the Rust mirror,
     *  rides the existing `dashboard.save` UPSERT — no new verb). Resolved through the ONE
     *  `resolveFieldPresentation` both the form and the table use. */
    hide?: boolean;
    /** PRESENTATION (widget-kit scope): an OPTIONAL order override for this field's column/position. Absent
     *  → the surface keeps its natural order (a table's first-seen/schema order). Never reorders implicitly. */
    order?: number;
    /** Grafana unit id (`celsius`/`bytes`/`percent`/`velocitykmh`/`time:…`). Mapped to a dimension by
     *  `fieldconfig/units.ts` and rendered through the user-prefs bridge (`fieldconfig/format.ts`). */
    unit?: string;
    decimals?: number;
    min?: number;
    max?: number;
    noValue?: string;
    /** CARRY (grafana-parity-ui P1): use the datasource-provided display name. Typed so an imported
     *  Grafana panel round-trips it verbatim; NOT rendered yet — the inspector flags it. */
    displayNameFromDS?: string;
    /** CARRY (grafana-parity-ui P1): Grafana's per-field table-filter toggle. Typed for round-trip;
     *  NOT rendered yet (table cellOptions land in P3) — the inspector flags it. */
    filterable?: boolean;
    mappings?: ValueMapping[];
    thresholds?: ThresholdsConfig;
    color?: FieldColor;
    /** Field data links (Grafana's `links`). Authored in the Field tab / as an override property. */
    links?: DataLink[];
    /** Per-view field options (lineWidth, fillOpacity, drawStyle, axisPlacement…). Grafana's
     *  `fieldConfig.custom`; the chart-types layer owns the per-view schema. */
    custom?: Record<string, unknown>;
}

/** One per-field override: a matcher + the properties it sets (Grafana's `DynamicConfigValue[]`,
 *  with dotted ids like `custom.lineWidth` accepted verbatim so import is a copy). */
export declare interface FieldOverride {
    matcher: Matcher;
    properties: Array<{
        id: string;
        value: unknown;
    }>;
}

/** The grid's column count — the geometry vocabulary every cell's `x`/`w` is written against. */
export declare const GRID_COLS = 12;

/** One grid row's pixel height (react-grid-layout `rowHeight`). */
export declare const GRID_ROW_PX = 56;

/** Is this row collapsed? (`options.collapsed === true`) — this doubles as the row's DEFAULT open/closed
 *  state: it's the stored collapse flag applied on load (panel-rows options). */
export declare function isCollapsed(cell: Cell): boolean;

/** Is this cell a row header? */
export declare function isRow(cell: Cell): boolean;

/** The slice of a react-grid-layout item the merge reads (avoids importing RGL types here). */
export declare interface LayoutItem {
    i: string;
    x: number;
    y: number;
    w: number;
    h: number;
}

/** A matcher selecting which fields an override applies to. The backend (`rust/crates/viz` config.rs)
 *  evaluates `byName`/`byType`/`byRegexp` (field matchers) + `byFrameRefID` (a frame matcher). The id
 *  spellings are the backend's VERBATIM — note `byRegexp` (Grafana's spelling), not `byRegex`. */
export declare interface Matcher {
    id: "byName" | "byType" | "byRegexp" | "byFrameRefID";
    /** `byName`: the field name; `byType`: `"number"|"string"|"time"|…`; `byRegexp`: the pattern;
     *  `byFrameRefID`: the target refId (A/B/…). */
    options?: unknown;
}

/** Merge a new layout (geometry only) back onto `cells`. Cells present in `next` take its
 *  geometry verbatim (the layout is authoritative for on-screen items); hidden members of a
 *  moved row shift by the row's Δy; everything else passes through unchanged. */
export declare function mergeLayout(cells: Cell[], next: LayoutItem[]): Cell[];

/** Per-panel query options (Grafana's query-options row). All optional — absent = defaults.
 *  The time-override fields are Grafana-verbatim; the host interprets them when dispatching
 *  targets — the grid only renders the badge. `hideTimeOverride` is display-only. */
export declare interface QueryOptions {
    maxDataPoints?: number;
    minInterval?: string;
    relativeTime?: string;
    /** Replaces the range with `[now - timeFrom, now]` for this panel (e.g. `"6h"`). */
    timeFrom?: string;
    /** Moves BOTH range ends earlier by this duration (e.g. `"1d"`) — a comparison offset. */
    timeShift?: string;
    /** Display-only: hide the override badge in the panel header. Never affects the query. */
    hideTimeOverride?: boolean;
}

/** A row header's height in grid units — a short bar, not a widget frame. */
export declare const ROW_H = 1;

/** The full width a row header spans — our grid is 12 columns (Grid.tsx `COLS`), so a row is 12 wide. */
export declare const ROW_W = 12;

export declare function RowHeader({ cell, memberCount, editable, onToggleCollapse, onRename, onRemove, }: RowHeaderProps): JSX_2.Element;

export declare interface RowHeaderProps {
    cell: Cell;
    /** How many positional members this row owns (shown beside the title). */
    memberCount: number;
    editable: boolean;
    /** Toggle `options.collapsed` (the persistence seam). Omitted ⇒ the chevron is inert. */
    onToggleCollapse?: (i: string) => void;
    /** Rename the row inline (the persistence seam). Omitted / non-editable ⇒ read-only title. */
    onRename?: (i: string, title: string) => void;
    /** Remove the row header (row-only delete). Omitted / non-editable ⇒ no remove affordance. */
    onRemove?: (i: string) => void;
}

/** The member cells of `row` — every NON-row cell whose `y` is ≥ the row's `y` and < the next row's `y`
 *  (positional membership). The next row is the row with the smallest `y` strictly greater than this
 *  one's; a trailing row owns everything below it. A row is never its own member. If `row` is not
 *  actually a row cell in `cells`, returns `[]` (a defensive no-op). */
export declare function rowMembers(cells: Cell[], row: Cell): Cell[];

/** A row header's presentation options, defaulted (panel-rows options). `showCount` = show the "· N
 *  panels" member count beside the title; `showLine` = draw the bottom divider line; `collapsed` = the
 *  stored default open/closed state. Both display flags default TRUE (today's look) so a pre-options row
 *  is unchanged; only an explicit `false` hides them. */
export declare interface RowOptions {
    showCount: boolean;
    showLine: boolean;
    collapsed: boolean;
}

/** Read a row cell's presentation options with defaults. A non-row cell reads as all-default. */
export declare function rowOptions(cell: Cell): RowOptions;

/** The row cells in a dashboard, ordered by `y` (then `x`) — the section boundaries. */
export declare function rows(cells: Cell[]): Cell[];

/** A v2 cell source — ANY MCP tool call (read or write); the HOST re-checks the grant per call. */
export declare interface Source {
    tool: string;
    args?: Record<string, unknown>;
}

/** A Grafana "target" — one query against one datasource. Generalizes the single {@link Source};
 *  `refId` (A,B,…) is referenced by transformations + overrides. A v2 single-`source` cell reads
 *  as `sources[0]` through {@link cellSources}. */
export declare interface Target {
    refId: string;
    datasource?: DataSourceRef;
    tool: string;
    args?: Record<string, unknown>;
    hide?: boolean;
}

/** Step coloring. The first step's `value` is always `null` (-∞), per Grafana. */
export declare interface ThresholdsConfig {
    mode: "absolute" | "percentage";
    steps: Array<{
        value: number | null;
        color: string;
    }>;
}

/** The badge text for a panel's time override, or null when none applies / it's hidden.
 *
 *  - `timeFrom` → the panel's own window (`"6h"` → "Last 6h").
 *  - `timeShift` → the comparison offset (`"1d"` → "1d earlier").
 *  - both → "Last 6h, 1d earlier".
 *  - `relativeTime` is the pre-Grafana vocabulary for the same idea (`"now-6h"`); shown when no
 *    `timeFrom` is set so an existing cell keeps announcing its override.
 *  `hideTimeOverride` → null (the author opted out of the badge, not out of the override). */
export declare function timeOverrideBadge(qo: QueryOptions | undefined): string | null;

/** The dashboard's active time window. ISO strings (`"2026-07-15"` or full timestamps) — the
 *  package never parses them; renderers hand them to their data client verbatim. */
export declare interface TimeRange {
    from: string;
    to: string;
}

/** Header-chrome visibility flags (each optional toolbar control is HIDDEN by default). */
export declare interface Toolbar {
    dateSelect?: boolean;
    refreshRate?: boolean;
    share?: boolean;
}

/** A client-side transformation (shape only — applying it is the consumer's). */
export declare interface Transformation {
    id: string;
    options?: Record<string, unknown>;
    disabled?: boolean;
    filter?: Matcher;
}

/** The cells that belong to NO row — those with a `y` above the first row (the ungrouped top-of-board
 *  region). A dashboard with no rows returns every non-row cell here. */
export declare function ungroupedCells(cells: Cell[]): Cell[];

/** The honest unknown-view placeholder — what the grid renders when the registry has no
 *  renderer for a cell's view. Says exactly what is missing; never throws, never guesses. */
export declare function UnknownView({ view }: {
    view: string;
}): JSX_2.Element;

/** A value mapping (Grafana's discriminated union). `value`/`range`/`special` render in Phase 1;
 *  `regex` is accepted but deferred (named follow-up) — it never silently mis-renders. */
export declare type ValueMapping = {
    type: "value";
    options: Record<string, ValueMappingResult>;
} | {
    type: "range";
    options: {
        from: number | null;
        to: number | null;
        result: ValueMappingResult;
    };
} | {
    type: "regex";
    options: {
        pattern: string;
        result: ValueMappingResult;
    };
} | {
    type: "special";
    options: {
        match: "true" | "false" | "null" | "nan" | "null+nan" | "empty";
        result: ValueMappingResult;
    };
};

/** The result of a value/range/regex/special mapping — text/color/icon to display instead of the raw value. */
export declare interface ValueMappingResult {
    text?: string;
    color?: string;
    icon?: string;
    index?: number;
}

/** The v2/v3 render vocabulary. Read views render a tool's result; scripted views run author
 *  code; control views call a write tool; `ext:<id>/<widget>` mounts an extension-shipped tile.
 *  v3 ADDS Grafana's panel-type ids as the canonical vocabulary (`timeseries`, `barchart`, …);
 *  the shipped v2 views remain valid ALIASES (`chart` → `timeseries`) via {@link canonicalView}.
 *  The PACKAGE ships no renderer for any of these — the consumer's registry does; the vocabulary
 *  lives here so every surface spells a view the same way. */
export declare type View = "chart" | "stat" | "gauge" | "table" | "plot" | "d3" | "template" | "switch" | "slider" | "button" | "json" | "jsonview" | "timeseries" | "barchart" | "bargauge" | "piechart" | "genui" | "insights" | "weather" | "text" | "row" | `ext:${string}`;

/** Asset-sharing visibility tiers. */
export declare type Visibility = "private" | "team" | "workspace";

/** The cells the grid should actually render, with collapsed rows' members hidden. A collapsed row's
 *  members are DROPPED from the render list (kept in the record); the row header itself always renders.
 *  This is the render-time transform (panel-rows scope, "collapse is a render-time transform") — it
 *  never mutates the stored geometry. Non-row cells and expanded rows pass through unchanged. */
export declare function visibleCells(cells: Cell[]): Cell[];

export declare interface WidgetRegistry<S = unknown> {
    /** Register a renderer for a view id (chainable). Later registrations win. */
    register(view: View | string, renderer: WidgetRenderer<S>): WidgetRegistry<S>;
    /** The renderer for a view id — exact canonical match, then the `ext:*` wildcard for
     *  `ext:` views, else `undefined` (the grid shows the honest placeholder). */
    resolve(view: View | string): WidgetRenderer<S> | undefined;
    /** The renderer for a CELL (resolves the cell's effective view first). */
    resolveCell(cell: Cell): WidgetRenderer<S> | undefined;
    /** The registered view ids (canonical spellings). */
    views(): string[];
}

export declare type WidgetRenderer<S = unknown> = ComponentType<WidgetRenderProps<S>>;

/** What every registered renderer receives. `scope` is the OPAQUE generic the consumer's
 *  variables machinery (or anything else) flows through — the package never reads it. */
export declare interface WidgetRenderProps<S = unknown> {
    cell: Cell;
    range?: TimeRange;
    scope?: S;
    /** Auto-refresh tick — bump to re-run read cells. Forwarded verbatim. */
    refreshKey?: number;
    /** Whether the hosting board is editable (some renderers dim controls when it is). */
    editable?: boolean;
}

/** The Phase-1 built-in widget types (v1). v2's render vocabulary is {@link View}. */
export declare type WidgetType = "chart" | "stat" | "gauge";

export { }
