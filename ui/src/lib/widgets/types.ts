// The Widget Kit types (widget-kit scope, Phase 1) — the relocated widget vocabulary + the per-field
// PRESENTATION vocabulary, the library's shared type home (FILE-LAYOUT: types only, no React). These
// moved out of `lib/channel/palette.types.ts` so any surface (palette, dashboard, channel responses,
// a future ext host) imports the widget contract from ONE place, not a palette-local file. The palette
// types file re-exports them for its existing importers (behavior-preserving move).

/** An `x-lb` entity hint — drives which `@`-lister an arg's picker is backed by. */
export type EntityKind = "datasource" | "channel" | "member" | "agent" | "table";

/** An `x-lb` widget hint — selects the arg widget the rail renders. The vocabulary is UI BUILT-INS ∪
 *  EXTENSION-CONTRIBUTED widgets, resolved by STRING: a built-in id (`sql`/`text`/`runtime`/`entity`/
 *  `select`/`number`/`boolean`/`date`/`cron`), an `ext:<id>/<widget>` id (an extension-contributed arg
 *  widget), or anything else (a newer author hint) that degrades to a plain text input. It is an OPEN
 *  string, not a closed enum — the UI has ZERO tool-specific knowledge and resolves any widget by name. */
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

/** The vendor-hint block under a form property's `x-lb` key (all fields optional). `options`/`source`
 *  feed a `select` widget; `showIf`/`requiredWhenShown` are the CONDITIONAL twin of the presentation
 *  keys (see the `/remind` slice). `label`/`description`/`hide`/`order` are the PER-FIELD PRESENTATION
 *  keys (widget-kit scope): they declare the field's identity (its name/help/visibility/order) on the
 *  request FORM, resolved through the SAME {@link FieldPresentation} resolver the response TABLE uses
 *  (`fieldConfig.displayName`/`description`/`hide`) — so a header and a form label never drift.
 *  `v` is the hint version (default 1) — a stamp so a future widget shape can be introduced additively. */
export interface XLbHint {
  entity?: EntityKind;
  widget?: WidgetKind;
  /** `select`: a static option list. */
  options?: string[];
  /** `select`: a catalog tool whose rows become the option list (fetched via the bridge, gated). */
  source?: string;
  /** CONDITIONAL VISIBILITY (generic form hint): this field is SHOWN only when every named arg in the
   *  map equals its declared value against the currently-collected form values. Absent → always shown. */
  showIf?: Record<string, string>;
  /** When the field is SHOWN (its {@link showIf} matches, or it has none), treat it as REQUIRED. */
  requiredWhenShown?: boolean;
  /** PRESENTATION: the field's display label — overrides the humanized name (`maxRuns` → "Max Runs").
   *  A label override always wins over the humanize fallback. */
  label?: string;
  /** PRESENTATION: help/tooltip text shown beside the field. */
  description?: string;
  /** PRESENTATION: omit this field from the rendered surface. This is PRESENTATION, NOT SECURITY — a
   *  hidden field still crossed the bridge under the viewer's grant; anything truly secret must be
   *  DENIED server-side, never hidden. See {@link FieldPresentation.hidden}. */
  hide?: boolean;
  /** PRESENTATION: an OPTIONAL order override. Absent → the surface keeps its natural order (the arg
   *  rail's required-first, a table's schema/first-seen order). Never reorders implicitly. */
  order?: number;
  /** The hint version (default 1). Additive — a new widget shape bumps this without breaking readers. */
  v?: number;
}

/** One JSON-Schema property (the subset the palette reads — `type` + the `x-lb` hints). */
export interface SchemaProperty {
  type?: string;
  "x-lb"?: XLbHint;
}

/** The subset of presentation hints a resolver reads from EITHER declaration site: an `x-lb` form hint
 *  (`label`/`description`/`hide`/`order`) OR a response `fieldConfig` FieldOptions (`displayName` ==
 *  label, `description`, `hide`). Kept structural so {@link resolveFieldPresentation} funnels BOTH the
 *  form and the table through one code path. `displayName` is accepted as an alias for `label` so the
 *  Grafana-adopted `fieldConfig` shape resolves without a copy. */
export interface FieldPresentationHints {
  label?: string;
  /** Grafana `fieldConfig` alias for {@link label} — `displayName` == label (widget-kit scope). */
  displayName?: string;
  description?: string;
  hide?: boolean;
  order?: number;
}

/** The RESOLVED presentation for one field — what every surface renders. `label` is the override or the
 *  humanized fallback (never empty); `hidden` drops the field from the surface; `order` is the optional
 *  override (undefined → the caller keeps the field's natural order).
 *
 *  `hidden` is PRESENTATION, NOT SECURITY. A hidden field was still returned by the tool and crossed the
 *  bridge under the VIEWER'S grant — hiding it removes it from a rendered surface, it does NOT gate
 *  access. Anything truly secret must be DENIED server-side (a denied source is denied whether or not a
 *  field is hidden); secrets are never merely hidden. */
export interface FieldPresentation {
  label: string;
  description?: string;
  hidden: boolean;
  order?: number;
}
