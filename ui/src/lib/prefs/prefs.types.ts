// The resolved-prefs shape the client consumes (user-prefs scope). Mirrors `lb_prefs::ResolvedPrefs`
// 1:1 — every axis decided by the server's resolution chain (request override → user → workspace
// default → built-in). The client never re-resolves the chain; it reads this and hands the presentation
// axes to `format.*`. The closed enums (`Dimension`/`Unit`) live in `dimensions.generated.ts` (the
// server-generated source of truth); the axes below are the small closed sets the formatter needs.

/** Date field order — matches `lb_prefs::DateStyle` (serde `snake_case`). */
export type DateStyle = "eu" | "iso" | "usa";

/** 12h/24h clock — matches `lb_prefs::TimeStyle` (serde `snake_case`). */
export type TimeStyle = "h12" | "h24";

/** A fully-resolved preference set for the viewer. Only the axes the client renders through are typed
 *  here; the server's record has more (language/number_format/unit_overrides) which pass through
 *  opaquely — the viz layer reads timezone + date/time style for `format.datetime`. */
export interface ResolvedPrefs {
  language: string;
  timezone: string;
  date_style: DateStyle;
  time_style: TimeStyle;
  first_day_of_week?: string;
  number_format?: string;
  unit_system?: string;
  unit_overrides?: Record<string, string>;
  /** The member's (or workspace-default's) UI theme — an OPAQUE blob owned by the theme layer
   *  (`lib/theme` parses it into a `ThemePreference`). Prefs stores/folds it whole; this layer never
   *  inspects its shape. `undefined` when neither the member nor the workspace set one. */
  ui_theme?: unknown;
}
