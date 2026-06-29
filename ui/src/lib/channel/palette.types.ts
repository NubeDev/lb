// The command-palette catalog wire types (channels-command-palette scope) — mirror the Rust
// `lb_mcp::ToolDescriptor` + `lb_host::ToolsCatalog` (rust/crates/mcp/src/registry.rs,
// rust/crates/host/src/tools/catalog.rs) ONE-TO-ONE. `tools.catalog` returns, for the calling
// principal in their workspace, ONLY the tools they are authorized to call — registered tools ∩
// caps held — each with a standard JSON-Schema `input_schema` carrying the two `x-lb` vendor hints
// the palette reads (`entity` → @-picker, `widget` → arg widget). Types only here (FILE-LAYOUT).

/** An `x-lb` entity hint — drives which `@`-lister an arg's picker is backed by. */
export type EntityKind = "datasource" | "channel" | "member" | "agent" | "table";

/** An `x-lb` widget hint — selects the arg widget the rail renders. */
export type WidgetKind = "sql" | "text";

/** The vendor-hint block under a property's `x-lb` key (both fields optional). */
export interface XLbHint {
  entity?: EntityKind;
  widget?: WidgetKind;
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

/** One authorized tool — mirrors `ToolDescriptor`. `input_schema` absent → a single free-text arg. */
export interface ToolDescriptor {
  name: string;
  title: string;
  group: string;
  input_schema?: InputSchema;
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
