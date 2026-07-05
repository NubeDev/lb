// The system-catalog MODEL — sections AS DATA (system-catalog scope). The picker model
// (`sourcePicker.ts`) answers "pick a source by label"; this answers "browse the workspace's
// subsystems as a tree." Both share the loader seam (`SourceLoaders`) and the row shapes in
// `types.ts`.
//
// A catalog SECTION is a kind (`"datasources" | "schema" | "series" | "channels" | "insights" |
// "inbox"`) plus its label/hint (registry-driven data — never a named case in the renderer). A
// catalog ENTRY is what a click yields — a tagged row the HOST maps onto whatever it persists
// (a Rhai snippet, a SQL table name, a dashboard cell source). The package never branches on host
// meaning (rule 10): it returns entries; the host maps them.
//
// PURE: no I/O, no transport, no React. The orchestration lives in `loadCatalog`; the tree skin in
// `CatalogExplorer`.

import type {
  ChannelRow,
  DatasourceRow,
  InboxRow,
  InsightRow,
  Schema,
} from "./types";

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
export type CatalogSectionKind =
  | "datasources"
  | "schema"
  | "series"
  | "channels"
  | "insights"
  | "inbox"
  | "extensions"
  | "rules"
  | "flowSummaries"
  | "flowDescriptors";

/** A section's declarative descriptor — its kind (loader-keyed), its human label, and a one-line
 *  hint. Exported as `CATALOG_SECTION_SPECS` (the canonical list); a host composes its surface by
 *  which loaders it wires (absent loader ⇒ absent section). */
export interface CatalogSectionSpec {
  kind: CatalogSectionKind;
  label: string;
  hint: string;
}

/** The canonical section registry. A host renders whichever of these its loaders cover; ids stay
 *  opaque (rule 10 — no core branch on a host's "known subsystem list"). */
export const CATALOG_SECTION_SPECS: CatalogSectionSpec[] = [
  {
    kind: "datasources",
    label: "Datasources",
    hint: "Registered external sources — click to query by name.",
  },
  {
    kind: "schema",
    label: "Local tables",
    hint: "Tables in this workspace's store — click to insert a name.",
  },
  {
    kind: "series",
    label: "Series",
    hint: "Discoverable timeseries — click to read 24h of history.",
  },
  {
    kind: "channels",
    label: "Channels",
    hint: "Registered channels in this workspace — click to reference one.",
  },
  {
    kind: "insights",
    label: "Insights",
    hint: "Open data findings — click to reference one.",
  },
  {
    kind: "inbox",
    label: "Inbox",
    hint: "Items in this channel's inbox — click to reference one.",
  },
];

/** What a click in the explorer yields — a tagged row the HOST maps onto its snippet/bind. Each kind
 *  carries ONLY the fields a host needs to form that mapping; the package owns no host semantics
 *  (rule 10). The host's `onSelect` is the one place "what this pick MEANS" is decided. */
export type CatalogEntry =
  | { kind: "datasource"; id: string; name: string; rowKind: string; endpoint?: string }
  | { kind: "table"; id: string; table: string }
  | { kind: "column"; id: string; table: string; column: string }
  | { kind: "series"; id: string; name: string }
  | { kind: "channel"; id: string; name: string }
  | { kind: "insight"; id: string; title: string; severity?: string; status?: string }
  | { kind: "inbox"; id: string; channel: string };

/** Datasource rows → catalog entries. The id is the name (stable round-trip key). */
export function datasourceEntries(rows: DatasourceRow[]): CatalogEntry[] {
  return rows.map((d) => ({
    kind: "datasource" as const,
    id: `datasource:${d.name}`,
    name: d.name,
    rowKind: d.kind,
    endpoint: d.endpoint,
  }));
}

/** Schema → table entries (one per table). Columns are addressed by the `column` kind via
 *  `schemaColumnEntries` (the explorer's table→column tree opens a table, then lists its columns). */
export function schemaTableEntries(schema: Schema): CatalogEntry[] {
  return schema.tables.map((t) => ({
    kind: "table" as const,
    id: `table:${t.name}`,
    table: t.name,
  }));
}

/** Schema → (table, column) entries — the columns of every table, flattened. The explorer's tree
 *  groups these under their table; the package exposes them flat so a host that wants a flat
 *  column picker can also consume them. */
export function schemaColumnEntries(schema: Schema): CatalogEntry[] {
  const out: CatalogEntry[] = [];
  for (const t of schema.tables) {
    for (const c of t.columns) {
      out.push({
        kind: "column" as const,
        id: `column:${t.name}.${c.name}`,
        table: t.name,
        column: c.name,
      });
    }
  }
  return out;
}

/** Series names → catalog entries (one per series). */
export function seriesCatalogEntries(names: string[]): CatalogEntry[] {
  return names.map((s) => ({ kind: "series" as const, id: `series:${s}`, name: s }));
}

/** Channel rows → catalog entries. */
export function channelEntries(rows: ChannelRow[]): CatalogEntry[] {
  return rows.map((c) => ({ kind: "channel" as const, id: `channel:${c.id}`, name: c.id }));
}

/** Insight rows → catalog entries. */
export function insightEntries(rows: InsightRow[]): CatalogEntry[] {
  return rows.map((i) => ({
    kind: "insight" as const,
    id: `insight:${i.id}`,
    title: i.title,
    severity: i.severity,
    status: i.status,
  }));
}

/** Inbox rows → catalog entries. */
export function inboxEntries(rows: InboxRow[]): CatalogEntry[] {
  return rows.map((i) => ({ kind: "inbox" as const, id: `inbox:${i.id}`, channel: i.channel }));
}
