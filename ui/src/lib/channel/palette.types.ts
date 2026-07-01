// The command-palette catalog wire types (channels-command-palette scope) — mirror the Rust
// `lb_mcp::ToolDescriptor` + `lb_host::ToolsCatalog` (rust/crates/mcp/src/registry.rs,
// rust/crates/host/src/tools/catalog.rs) ONE-TO-ONE. `tools.catalog` returns, for the calling
// principal in their workspace, ONLY the tools they are authorized to call — registered tools ∩
// caps held — each with a standard JSON-Schema `input_schema` carrying the two `x-lb` vendor hints
// the palette reads (`entity` → @-picker, `widget` → arg widget). Types only here (FILE-LAYOUT).

import type { RichResultPayload } from "./payload.types";

/** An `x-lb` entity hint — drives which `@`-lister an arg's picker is backed by. */
export type EntityKind = "datasource" | "channel" | "member" | "agent" | "table";

/** An `x-lb` widget hint — selects the arg widget the rail renders. The vocabulary is UI BUILT-INS ∪
 *  EXTENSION-CONTRIBUTED widgets, resolved by STRING: a built-in id (`sql`/`text`/`runtime`/`entity`/
 *  `select`/`number`/`boolean`/`date`/`cron`), an `ext:<id>/<widget>` id (an extension-contributed arg
 *  widget), or anything else (a newer author hint) that degrades to a plain text input. It is an OPEN
 *  string, not a closed enum — the UI has ZERO tool-specific knowledge and resolves any widget by name.
 *  `runtime` drives the agent command's runtime dropdown (external-agent run-lifecycle #5, fed by
 *  `agent.runtimes`). The rich-response widgets ADD `select`/`number`/`boolean`/`date`/`cron`. An UNKNOWN
 *  widget falls back to text (never crashes) — the registry resolves it, so a newer or extension hint
 *  degrades gracefully on an older UI. */
export type BuiltinWidgetKind =
  | "sql"
  | "text"
  | "runtime"
  | "entity"
  | "select"
  | "number"
  | "boolean"
  | "date"
  | "cron";

/** The wire type of an `x-lb.widget` hint — a built-in id, an `ext:<id>/<widget>` id, or any string a
 *  newer author emitted. Kept OPEN (a plain `string`) so the registry, not the type, is the vocabulary. */
export type WidgetKind = BuiltinWidgetKind | (string & {});

/** The vendor-hint block under a property's `x-lb` key (all fields optional). `options`/`source` feed a
 *  `select` widget: a static option list, or a catalog tool whose rows become options (fetched, gated).
 *  `v` is the hint version (default 1) — a stamp so a future widget shape can be introduced additively. */
export interface XLbHint {
  entity?: EntityKind;
  widget?: WidgetKind;
  /** `select`: a static option list. */
  options?: string[];
  /** `select`: a catalog tool whose rows become the option list (fetched via the bridge, gated). */
  source?: string;
  /** The hint version (default 1). Additive — a new widget shape bumps this without breaking readers. */
  v?: number;
}

/** One JSON-Schema property (the subset the palette reads — `type` + the `x-lb` hints). */
export interface SchemaProperty {
  type?: string;
  "x-lb"?: XLbHint;
}

/** A standard JSON Schema for a tool's input object (the subset the palette uses). */
export interface InputSchema {
  type?: string;
  properties?: Record<string, SchemaProperty>;
  required?: string[];
}

/** The render-envelope a descriptor may DECLARE for its result (`ToolDescriptor.result`) — the
 *  `x-lb-render` envelope, the {@link RichResultPayload} shape MINUS the wire `kind`/`v` tags (the
 *  palette stamps those via `encodeRichResult`). When a descriptor carries this, the palette POSTS this
 *  render (with the collected args interpolated into `source.args`) instead of showing a raw tool result.
 *  The UI reads it purely by shape — it never branches on the tool name. Mirrors the Rust descriptor's
 *  optional `result`. */
export type RenderEnvelope = Omit<RichResultPayload, "kind" | "v"> & { v?: 2 };

/** One authorized tool — mirrors `ToolDescriptor`. `input_schema` absent → a single free-text arg.
 *  `result` (optional) is the descriptor-declared render envelope — present → the palette posts it. */
export interface ToolDescriptor {
  name: string;
  title: string;
  group: string;
  input_schema?: InputSchema;
  result?: RenderEnvelope;
}

/** The `tools.catalog` response — the caller's authorized tool set for their workspace. */
export interface ToolsCatalog {
  ws: string;
  tools: ToolDescriptor[];
}

/** Read the `x-lb` hint for a property of a tool's schema (undefined when absent). */
export function hintFor(schema: InputSchema | undefined, key: string): XLbHint | undefined {
  return schema?.properties?.[key]?.["x-lb"];
}

/** The ordered arg names of a tool's schema (the rail fills them left→right). */
export function argNames(schema: InputSchema | undefined): string[] {
  if (!schema?.properties) return [];
  // `required` first (stable, matches the schema author's intent), then any remaining props.
  const req = schema.required ?? [];
  const rest = Object.keys(schema.properties).filter((k) => !req.includes(k));
  return [...req.filter((k) => k in schema.properties!), ...rest];
}
