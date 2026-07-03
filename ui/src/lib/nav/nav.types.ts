// The nav wire shapes — mirror the gateway's `nav.*` routes + the host `Nav` record (nav scope). A
// nav is a persisted, ordered menu of items, each linking to a core surface, a specific dashboard, an
// (opaque) extension page, or a dynamic tag-group. The nav is a LENS over existing access — it grants
// nothing; `nav.resolve` returns the caller's effective menu, already tag-expanded and cap-stripped.

/** The S4 asset-sharing visibility tiers (identical to a dashboard's). */
export type Visibility = "private" | "team" | "workspace";

/** The four entry kinds plus one level of `group` (nav scope, "Four entry kinds"). */
export type NavItemKind = "surface" | "dashboard" | "ext" | "tag-group" | "group";

/** One faceted tag query on a `tag-group` item — `{ key, value? }`. A value present is exact; absent
 *  is key-only. Opaque data (never branched on). */
export interface NavFacet {
  key: string;
  value?: unknown;
}

/** One authored nav entry. Exactly one kind is meaningful per item; a `group` nests one level. The
 *  target references (`surface` key, `dashboard:{id}`, opaque `ext` id, `facets`) are opaque data. */
export interface NavItem {
  kind: NavItemKind;
  label?: string;
  /** `surface`: the core surface key (`"channels"`, `"rules"`, …). */
  surface?: string;
  /** `dashboard`: the `dashboard:{id}` reference. */
  dashboard?: string;
  /** `ext`: the OPAQUE extension id (never branched on — rule 10). */
  ext?: string;
  /** `tag-group`: the facets the dynamic entry expands over. */
  facets?: NavFacet[];
  /** `group`: the nested items (one level). */
  items?: NavItem[];
}

/** A full nav record (the menu + sharing metadata). */
export interface Nav {
  id: string;
  title: string;
  owner: string;
  visibility: Visibility;
  items: NavItem[];
  schemaVersion?: number;
  updated_ts: number;
  deleted?: boolean;
}

/** The cheap roster row `nav.list` returns (no item bodies). */
export interface NavSummary {
  id: string;
  title: string;
  visibility: Visibility;
  updated_ts: number;
}

/** Which tier `nav.resolve` picked the effective menu from. */
export type ResolvedSource = "pick" | "team" | "workspace-default" | "fallback";

/** One resolved menu entry — a `NavItem` after tag-expansion + cap-strip. A `tag-group` becomes a
 *  `group` of `dashboard` items; a `group` carries its (independently stripped) children. `label` is
 *  always populated (derived when the author left it empty). */
export interface ResolvedItem {
  kind: NavItemKind;
  label: string;
  surface?: string;
  dashboard?: string;
  ext?: string;
  items?: ResolvedItem[];
}

/** The `nav.resolve` payload — the caller's effective menu. On `fallback`, `items` is empty and the
 *  UI renders its built-in `SURFACES` (never a blank rail). */
export interface ResolvedNav {
  source: ResolvedSource;
  nav_id?: string;
  title?: string;
  items: ResolvedItem[];
}

/** The member's active-nav pick record (`nav_pref`). `active` empty = no pick. */
export interface NavPref {
  active: string;
  updated_ts: number;
}
