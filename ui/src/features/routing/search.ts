// Route search-param schemas for the shell URL grammar (routing scope). These validators are
// deliberately small and hand-written: malformed shared links degrade to defaults, never exceptions.

export const DEFAULT_CHANNEL = "general";

export interface ChannelSearch {
  c: string;
}

/** The schema-designer canvas search. `from` is an optional datasource name: when present (a user
 *  opened the designer from a datasource's Schemas tab), the canvas presets that source and
 *  auto-imports its catalog — the user never has to pick a source from the dropdown. */
export interface SchemaSearch {
  from?: string;
}

export function validateSchemaSearch(search: Record<string, unknown>): SchemaSearch {
  const from = scalar(search.from)?.trim();
  return from ? { from } : {};
}

// The dashboard search carries the date range, the auto-refresh interval (Slice 4), and the selected
// variable values (Slice 2) as flat `var-<name>` URL params (repeated for multi-value). Selection lives
// in the URL — per-viewer + shareable (Grafana parity); the variable DEFINITIONS live on the record. We
// keep `var-<name>` as literal keys (not a nested object) so TanStack round-trips them in the URL; the
// `varsFromSearch`/`withVars` helpers translate to/from a bare-name map.
export interface DashboardSearch {
  from: string;
  to: string;
  /** The selected dashboard id, URL `?d=<id>`. Lives in the URL so a pasted link re-opens the same
   *  dashboard (shareable, Grafana parity). Absent = show the roster, no dashboard selected. */
  d?: string;
  /** The auto-refresh interval (widget-config-vars Slice 4), URL `?refresh=30s`. Absent/`""` = off. */
  refresh?: string;
  /** Flat `var-<name>` selection params (one string, or a string[] for a multi-value selection). */
  [key: `var-${string}`]: string | string[] | undefined;
}

/** The accepted refresh intervals (widget-config-vars Slice 4). `""` = off. */
export const REFRESH_OPTIONS = ["", "5s", "10s", "30s", "1m", "5m", "15m"] as const;
const REFRESH_SET = new Set<string>(REFRESH_OPTIONS);

function scalar(value: unknown): string | null {
  if (typeof value === "string") return value;
  if (Array.isArray(value) && typeof value[0] === "string") return value[0];
  return null;
}

export function validateChannelSearch(search: Record<string, unknown>): ChannelSearch {
  const c = scalar(search.c)?.trim();
  return { c: c || DEFAULT_CHANNEL };
}

function isoDate(value: unknown): string | null {
  const raw = scalar(value);
  if (!raw || !/^\d{4}-\d{2}-\d{2}$/.test(raw)) return null;
  const d = new Date(`${raw}T00:00:00.000Z`);
  return Number.isNaN(d.getTime()) || d.toISOString().slice(0, 10) !== raw ? null : raw;
}

function addDays(date: Date, days: number): Date {
  const next = new Date(date);
  next.setUTCDate(next.getUTCDate() + days);
  return next;
}

export function defaultDashboardSearch(today = new Date()): DashboardSearch {
  const end = new Date(Date.UTC(today.getUTCFullYear(), today.getUTCMonth(), today.getUTCDate()));
  return {
    from: addDays(end, -30).toISOString().slice(0, 10),
    to: end.toISOString().slice(0, 10),
  };
}

export function validateDashboardSearch(search: Record<string, unknown>): DashboardSearch {
  const fallback = defaultDashboardSearch();
  const from = isoDate(search.from) ?? fallback.from;
  const to = isoDate(search.to) ?? fallback.to;
  const range = from <= to ? { from, to } : fallback;

  const out: DashboardSearch = { ...range };

  // The selected dashboard id — carried through as-is (a bad id just fails the load, never throws).
  const d = scalar(search.d)?.trim();
  if (d) out.d = d;

  // The refresh interval — keep only a known option; anything else degrades to off (drop the key).
  const refresh = scalar(search.refresh)?.trim();
  if (refresh && REFRESH_SET.has(refresh)) out.refresh = refresh;

  // Carry every `var-<name>` param through as-is (string or string[]), dropping empties. Malformed
  // shapes (a non-string, non-string[] value) are simply ignored — a shared link never throws.
  for (const [k, v] of Object.entries(search)) {
    if (!k.startsWith("var-")) continue;
    if (typeof v === "string") {
      out[k as `var-${string}`] = v;
    } else if (Array.isArray(v)) {
      const list = v.filter((x): x is string => typeof x === "string");
      if (list.length === 1) out[k as `var-${string}`] = list[0];
      else if (list.length > 1) out[k as `var-${string}`] = list;
    }
  }
  return out;
}

/** Extract the selected variable values from a dashboard search, keyed by BARE variable name (drops the
 *  `var-` prefix). A repeated param is a multi-value `string[]`; a single param is a `string`. */
export function varsFromSearch(search: DashboardSearch): Record<string, string | string[]> {
  const out: Record<string, string | string[]> = {};
  for (const [k, v] of Object.entries(search)) {
    if (k.startsWith("var-") && v !== undefined) out[k.slice(4)] = v as string | string[];
  }
  return out;
}

/** Return a new search with the variable `name` set to `value` (an empty value / empty array clears it).
 *  Used by the variable bar's dropdowns to write a selection to the URL. */
export function withVar(
  search: DashboardSearch,
  name: string,
  value: string | string[] | undefined,
): DashboardSearch {
  const key = `var-${name}` as const;
  const next: DashboardSearch = { ...search };
  const empty =
    value === undefined ||
    value === "" ||
    (Array.isArray(value) && value.length === 0);
  if (empty) delete next[key];
  else next[key] = value;
  return next;
}
