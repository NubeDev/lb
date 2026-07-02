// The x-lb widget registry (channel rich responses scope) — the ONE place the palette resolves an
// arg's `x-lb.widget` hint to a concrete arg widget. The vocabulary is OPEN: UI BUILT-INS ∪
// EXTENSION-CONTRIBUTED widgets, resolved by STRING (the UI has ZERO tool-specific knowledge). A hint
// resolves to a registered built-in {@link WidgetEntry}, an `ext:<id>/<widget>` EXTENSION entry, or —
// for an absent/unknown widget — the `text` fallback (never crashes). Types + resolution only (no React)
// so the palette imports a small table, not a switch (FILE-LAYOUT). The widget COMPONENTS live one-per-
// file in this folder; the palette renders them by the resolved `kind`.
//
// `inline`: a widget filled IN PLACE that stays active until submit (sql/runtime/select/cron/boolean —
// no chip), vs a chip widget the rail commits to a chip (text/number/date/entity). This is the same
// distinction the palette drew for sql/runtime; the registry now carries it as data.
//
// Arg-side ext widgets (the DECISION): the shipped ext-widget federation mount (`loadRemoteWidgetMount`/
// `mountWidget`, mirrored from the RESPONSE side's WidgetView→ExtWidget) is a bridge-driven, self-owned
// tile — its `RemoteWidgetMount` contract has NO value/onChange callback, so a federated ARG widget can't
// report a collected form value back to the palette through the shipped contract. So we resolve `ext:`
// arg hints to an `ext` entry (OPEN, no crash) that carries the `viewKey`, and the arg renderer mounts it
// through a thin ExtArg that falls back to a plain text input for value collection. RESPONSE-side ext
// widgets are unaffected — they already mount for real via WidgetView→ExtWidget (a self-owned read tile,
// no value-reporting needed). When the mount contract gains a value channel, ExtArg upgrades in place.

import type { WidgetKind, XLbHint } from "./types";

/** The concrete widget a hint resolves to — the registered built-in kinds plus `ext` (an extension-
 *  contributed arg widget carried by `viewKey`). */
export type WidgetId =
  | "sql"
  | "runtime"
  | "entity"
  | "select"
  | "cron"
  | "boolean"
  | "number"
  | "date"
  | "text"
  | "ext";

/** One registry entry. `inline` = filled in place (stays active until submit, no chip); otherwise the
 *  rail commits the value to a chip. An `ext` entry also carries the `ext:<id>/<widget>` `viewKey`. */
export interface WidgetEntry {
  kind: WidgetId;
  inline: boolean;
  /** For an `ext` entry only — the `ext:<id>/<widget>` key the arg renderer mounts. */
  viewKey?: string;
}

/** The built-in registry: each built-in `x-lb.widget` id → its widget entry. `sql`/`runtime`/`entity`
 *  keep the shipped behavior (inline sql/runtime, chipped entity picker). The rich-response widgets are
 *  additive. An ENTITY arg (no `widget`, an `entity` hint) is handled by the palette's picker directly —
 *  the registry still lists `entity` so an explicit `widget:"entity"` resolves too. */
const REGISTRY: Record<string, WidgetEntry> = {
  sql: { kind: "sql", inline: true },
  runtime: { kind: "runtime", inline: true },
  entity: { kind: "entity", inline: false },
  select: { kind: "select", inline: true },
  cron: { kind: "cron", inline: true },
  boolean: { kind: "boolean", inline: true },
  number: { kind: "number", inline: false },
  date: { kind: "date", inline: false },
  text: { kind: "text", inline: false },
};

/** The fallback for an absent/unknown widget — a plain text input (never crashes). */
export const TEXT_ENTRY: WidgetEntry = REGISTRY.text;

/** Resolve an `x-lb` hint to a widget entry — the OPEN vocabulary. An `entity` hint (with or without a
 *  `widget`) resolves to the entity picker. An `ext:<id>/<widget>` widget resolves to an `ext` entry
 *  (carrying the viewKey — the arg renderer mounts the extension widget). A known built-in resolves to
 *  its entry. An unknown/absent `widget` → the `text` fallback (never crashes). */
export function resolveWidget(hint: XLbHint | undefined): WidgetEntry {
  if (hint?.entity) return REGISTRY.entity;
  const w: WidgetKind | undefined = hint?.widget;
  if (typeof w === "string" && w.startsWith("ext:")) {
    return { kind: "ext", inline: false, viewKey: w };
  }
  if (typeof w === "string" && w in REGISTRY) return REGISTRY[w];
  return TEXT_ENTRY;
}
