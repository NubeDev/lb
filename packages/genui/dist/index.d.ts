import { JSX as JSX_2 } from 'react';
import { Provider } from 'react';
import { ReactNode } from 'react';

export declare function applyPatch(spec: IrSpec, patch: Patch): IrSpec;

/** A binding to a value in the surface's data model, addressed by JSON Pointer (RFC 6901).
 *  A component prop is either a literal or one of these — `resolveBindings` swaps `$bind` for the
 *  pointed-at value at render time. `$bind` (not bare `{path}`) so a literal object prop can never be
 *  mistaken for a binding. */
export declare interface Binding {
    $bind: string;
}

export declare interface Catalog {
    entries: CatalogEntry[];
    /** Resolve a (possibly deprecated) component name to a live entry, or `undefined`. */
    resolve: (name: string) => CatalogEntry | undefined;
    /** The set of every LIVE component name (not aliases) — what `toJson`/the host validate against. */
    names: () => string[];
    /** True if `name` resolves (live or via a deprecated alias). */
    has: (name: string) => boolean;
}

export declare interface CatalogEntry {
    name: string;
    description: string;
    props: Record<string, PropSpec>;
    /** Catalog action names this component can `emit` (button/slider/switch); used by the prompt. */
    actions?: string[];
    render: (rp: RenderProps) => ReactNode;
    /** Old names/prop-names that map forward to THIS entry (the drift rule). */
    deprecatedAliases?: string[];
}

export declare interface CatalogJson {
    /** IR schema version this catalog targets — the host cross-checks a spec's `v` against known ones. */
    v: number;
    components: CatalogJsonComponent[];
}

export declare interface CatalogJsonComponent {
    name: string;
    description: string;
    props: Record<string, CatalogJsonProp>;
    actions?: string[];
    deprecatedAliases?: string[];
}

declare interface CatalogJsonProp {
    type: PropSpec["type"];
    description?: string;
    required?: boolean;
    values?: string[];
}

/** The flat name-set (live names only) — what the host membership-checks against. Sorted, stable. */
export declare function catalogNames(catalog: Catalog): string[];

export declare function catalogPrompt(catalog: Catalog): string;

/** One component instance in the flat map. `children` names other component ids (never inline nodes),
 *  so the tree is id-referenced and orphan-tolerant. `component` must resolve in the catalog. */
export declare interface Component {
    id: string;
    component: string;
    props?: Record<string, PropValue>;
    children?: string[];
}

/** The per-surface data model: an arbitrary JSON tree that bindings point into. The dashboard tenant
 *  keys it by refId under `/data/{refId}` (e.g. `/data/A/rows`), but the IR itself is agnostic. */
export declare type DataModel = Record<string, unknown>;

export declare function defineCatalog(entries: CatalogEntry[]): Catalog;

/** An empty spec — the starting point a stream folds patches into. */
export declare function emptySpec(surfaceId?: string): IrSpec;

export declare function errors(findings: Finding[]): Finding[];

/** A single validate/normalize finding surfaced to the AUTHOR (never a viewer). `level:"error"` blocks
 *  accept; `level:"warning"` is a fixed-up sloppiness the author is shown before saving. */
export declare interface Finding {
    level: "warning" | "error";
    code: string;
    message: string;
    componentId?: string;
}

/** The bridge-shaped seam the host injects — the SAME shape as the widget bridge (`makeWidgetBridge`),
 *  so the dashboard host passes its bridge straight through. `call` is host-re-checked per invocation
 *  against the cell leash; the token never enters this layer. */
export declare interface GenUiBridge {
    call: (tool: string, args?: Record<string, unknown>) => Promise<unknown>;
}

export declare interface GenUiCtx {
    bridge?: GenUiBridge;
}

export declare const GenUiProvider: Provider<GenUiCtx>;

export declare function GenUiSurface({ spec, data, catalog, bridge, onAction }: GenUiSurfaceProps): JSX_2.Element;

export declare interface GenUiSurfaceProps {
    spec: IrSpec;
    data?: DataModel;
    catalog: Catalog;
    /** The bridge the host injects for control actions (dashboard: the widget iframe bridge). */
    bridge?: GenUiBridge;
    /** Called for every control action BEFORE the bridge call — lets the host log/leash-check. The
     *  catalog entry's declared `tool` for the action (if any) is looked up by the host from the cell. */
    onAction?: (action: UiAction) => void;
}

/** The current IR schema version. `migrate.ts` upgrades older persisted specs forward on load. */
export declare const IR_VERSION: 1;

/** The whole persisted spec: version + surface + the flat component map + an OPTIONAL seed data model
 *  (authoring-time sample; steady-state data arrives via patches, never persisted). */
export declare interface IrSpec {
    v: number;
    surface: Surface;
    components: Record<string, Component>;
    dataModel?: DataModel;
}

export declare function isBinding(v: unknown): v is Binding;

/** Upgrade `spec` to the current IR version. Unknown-future versions are returned untouched (validate
 *  will flag them); a missing/zero `v` is treated as the earliest (v1) shape. */
export declare function migrate(spec: IrSpec): IrSpec;

export declare const nubeCatalog: Catalog;

/** The four typed patch messages (A2UI-shaped). `applyPatch` folds one into an `IrSpec`. */
export declare type Patch = {
    type: "createSurface";
    surface: Surface;
    components: Record<string, Component>;
    dataModel?: DataModel;
} | {
    type: "updateComponents";
    components: Component[];
} | {
    type: "updateDataModel";
    pointer: string;
    value: unknown;
} | {
    type: "deleteSurface";
    surfaceId: string;
};

/** A JSON-Schema-ish prop descriptor. Kept deliberately small — enough to generate a signature line and
 *  to type-coerce in normalize; not a full validator. `type` drives the coercion in normalize. */
export declare interface PropSpec {
    type: "string" | "number" | "boolean" | "array" | "object" | "enum" | "binding";
    description?: string;
    required?: boolean;
    /** For `type:"enum"`. */
    values?: string[];
    default?: unknown;
}

/** A prop value: any JSON literal, a binding, or a (possibly nested) array/object of the same. */
export declare type PropValue = string | number | boolean | null | Binding | PropValue[] | {
    [k: string]: PropValue;
};

/** The props a catalog entry's render fn receives: resolved (bindings already swapped) props, the
 *  resolved children (already-rendered React nodes, in `children` order), and the action dispatch. */
export declare interface RenderProps {
    props: Record<string, unknown>;
    children: ReactNode[];
    /** Emit a control action over the leashed bridge. `name` is the entry's action; `context` the args. */
    emit: (name: string, context?: Record<string, unknown>) => void;
}

/** Resolve a whole props bag. */
export declare function resolveBindings(props: Record<string, PropValue> | undefined, data: DataModel): Record<string, unknown>;

/** Resolve a single JSON Pointer against `data`. `""` = the whole document. Returns `undefined` for any
 *  missing segment (orphan-tolerant). Handles `~1`→`/` and `~0`→`~` unescaping and numeric array
 *  indices. */
export declare function resolvePointer(data: unknown, pointer: string): unknown;

/** Deep-resolve every `{$bind}` in a prop value against the data model. Literals pass through. */
export declare function resolveValue(value: PropValue, data: DataModel): unknown;

/** The addressable render root. `root` names the top component id; the tree is walked from there. */
export declare interface Surface {
    surfaceId: string;
    root: string;
}

export declare function toCatalogJson(catalog: Catalog, v: number): CatalogJson;

/** A typed action a control emits back over the bridge — `name` is the catalog action, `tool` the
 *  MCP verb the host re-checks against the cell leash, `context` the resolved args. */
export declare interface UiAction {
    surfaceId: string;
    componentId: string;
    name: string;
    tool: string;
    context?: Record<string, unknown>;
}

export declare function useGenUiBridge(): GenUiBridge | undefined;

export declare function validate(spec: IrSpec, opts: ValidateOptions): Finding[];

declare interface ValidateOptions {
    catalog: Catalog;
}

export declare function warnings(findings: Finding[]): Finding[];

export { }
