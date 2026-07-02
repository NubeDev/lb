import { ElementNode } from '@openuidev/lang-core';
import { LibraryJSONSchema } from '@openuidev/lang-core';
import { ReactNode } from 'react';

/** Run the loud accept step on an ALREADY-TYPED IR (the headless direct-IR choreography — no Lang round
 *  trip). Migrates, normalizes, validates, size-checks. */
export declare function acceptIr(ir: IrSpec, opts: AcceptOptions): AcceptResult;

/** Run the loud accept step on RAW OpenUI-Lang emission text. Parse → normalize → validate → size-check.
 *  Returns the typed IR to persist, or a loud rejection with the stated message. */
export declare function acceptLang(text: string, opts: AcceptOptions): AcceptResult;

export declare interface AcceptOptions {
    catalog: Catalog;
    surfaceId?: string;
    /** Enforce the size bound (default true). The builder passes the whole `options.genui` size; when only
     *  the IR is measured here, the caller may add the meta overhead itself. */
    maxBytes?: number;
}

export declare interface AcceptResult {
    ok: boolean;
    ir?: IrSpec;
    /** All findings (warnings shown in preview + the errors that blocked, if any). */
    findings: Finding[];
    /** The single stated rejection message when `ok` is false. */
    error?: string;
}

/** A binding to a value in the surface's data model, addressed by JSON Pointer (RFC 6901).
 *  A component prop is either a literal or one of these — `resolveBindings` swaps `$bind` for the
 *  pointed-at value at render time. `$bind` (not bare `{path}`) so a literal object prop can never be
 *  mistaken for a binding. */
declare interface Binding {
    $bind: string;
}

/** Build the parser schema from the catalog. One `$defs` entry per LIVE catalog component (keyed by its
 *  lang PascalCase name), plus a synthetic `Surface` root that accepts arbitrary children. */
export declare function buildLangLibrary(catalog: Catalog): LibraryJSONSchema;

declare interface Catalog {
    entries: CatalogEntry[];
    /** Resolve a (possibly deprecated) component name to a live entry, or `undefined`. */
    resolve: (name: string) => CatalogEntry | undefined;
    /** The set of every LIVE component name (not aliases) — what `toJson`/the host validate against. */
    names: () => string[];
    /** True if `name` resolves (live or via a deprecated alias). */
    has: (name: string) => boolean;
}

declare interface CatalogEntry {
    name: string;
    description: string;
    props: Record<string, PropSpec>;
    /** Catalog action names this component can `emit` (button/slider/switch); used by the prompt. */
    actions?: string[];
    render: (rp: RenderProps) => ReactNode;
    /** Old names/prop-names that map forward to THIS entry (the drift rule). */
    deprecatedAliases?: string[];
}

/** catalog name (`stat`, `barchart`) → lang name (`Stat`, `BarChart`). */
export declare function catalogToLangName(name: string): string;

/** One component instance in the flat map. `children` names other component ids (never inline nodes),
 *  so the tree is id-referenced and orphan-tolerant. `component` must resolve in the catalog. */
declare interface Component {
    id: string;
    component: string;
    props?: Record<string, PropValue>;
    children?: string[];
}

export declare function createLangStream(catalog: Catalog, surfaceId?: string): LangStream;

/** The per-surface data model: an arbitrary JSON tree that bindings point into. The dashboard tenant
 *  keys it by refId under `/data/{refId}` (e.g. `/data/A/rows`), but the IR itself is agnostic. */
declare type DataModel = Record<string, unknown>;

/** Convert a parsed lang root `ElementNode` into a full `IrSpec`. `surfaceId` names the surface (the
 *  cell id for the dashboard tenant). A null root yields an empty spec (mid-stream / unparseable). */
export declare function elementToIr(root: ElementNode | null, surfaceId?: string): IrSpec;

/** A single validate/normalize finding surfaced to the AUTHOR (never a viewer). `level:"error"` blocks
 *  accept; `level:"warning"` is a fixed-up sloppiness the author is shown before saving. */
declare interface Finding {
    level: "warning" | "error";
    code: string;
    message: string;
    componentId?: string;
}

/** The persisted-block size bound (genui-scope: "~8 KB"). The whole `options.genui` block (IR + meta) is
 *  bounded; an over-budget spec is almost certainly a bad generation and is rejected at accept AND at
 *  save. Kept here as the single source of truth the builder and tests share; the host mirrors it. */
export declare const GENUI_MAX_BYTES: number;

/** The whole persisted spec: version + surface + the flat component map + an OPTIONAL seed data model
 *  (authoring-time sample; steady-state data arrives via patches, never persisted). */
declare interface IrSpec {
    v: number;
    surface: Surface;
    components: Record<string, Component>;
    dataModel?: DataModel;
}

/** lang name (`Stat`, `BarChart`) → catalog name (`stat`, `barchart`). Built from the same table so the
 *  two directions can never drift. Falls back to lowercasing the first char. */
export declare function langNameToCatalog(langName: string): string;

/** The root STATEMENT id OpenUI Lang uses as the entry point. Lang is statement-based
 *  (`id = Component(...)`, one per line) and the parser's entry point is the statement named `root`
 *  (`DEFAULT_ROOT_STATEMENT_ID` in lang-core) — this is a statement id, NOT a component name, so no
 *  synthetic root component is needed. `createParser(schema, langRootName())` pins it explicitly. */
export declare function langRootName(): string;

export declare interface LangStream {
    /** Feed the next stream chunk (a `text-delta`), get the latest IR. */
    push: (chunk: string) => IrSpec;
    /** Set the full accumulated text (diffs internally). Use when the caller holds the whole buffer. */
    set: (fullText: string) => IrSpec;
    /** Latest IR without consuming new input. */
    current: () => IrSpec;
}

export declare function normalize(spec: IrSpec, catalog: Catalog): NormalizeResult;

export declare interface NormalizeResult {
    spec: IrSpec;
    findings: Finding[];
}

export declare function parseLang(text: string, catalog: Catalog, surfaceId?: string): ParseResultIr;

export declare interface ParseResultIr {
    ir: IrSpec;
    findings: Finding[];
}

/** The synthetic component an unknown name is rewritten to. The catalog SHOULD define a `placeholder`
 *  entry; if it doesn't, the surface renders the inert `gu-unknown` fallback — either way, no throw. */
export declare const PLACEHOLDER = "placeholder";

/** A JSON-Schema-ish prop descriptor. Kept deliberately small — enough to generate a signature line and
 *  to type-coerce in normalize; not a full validator. `type` drives the coercion in normalize. */
declare interface PropSpec {
    type: "string" | "number" | "boolean" | "array" | "object" | "enum" | "binding";
    description?: string;
    required?: boolean;
    /** For `type:"enum"`. */
    values?: string[];
    default?: unknown;
}

/** A prop value: any JSON literal, a binding, or a (possibly nested) array/object of the same. */
declare type PropValue = string | number | boolean | null | Binding | PropValue[] | {
    [k: string]: PropValue;
};

/** The props a catalog entry's render fn receives: resolved (bindings already swapped) props, the
 *  resolved children (already-rendered React nodes, in `children` order), and the action dispatch. */
declare interface RenderProps {
    props: Record<string, unknown>;
    children: ReactNode[];
    /** Emit a control action over the leashed bridge. `name` is the entry's action; `context` the args. */
    emit: (name: string, context?: Record<string, unknown>) => void;
}

/** The byte size of a spec as it will be persisted (JSON). */
export declare function specByteSize(ir: IrSpec): number;

/** The addressable render root. `root` names the top component id; the tree is walked from there. */
declare interface Surface {
    surfaceId: string;
    root: string;
}

export { }
