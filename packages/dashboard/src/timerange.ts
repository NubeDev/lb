// The package-owned time-range vocabulary — the cut for the shell's URL `DashboardSearch`
// entanglement. The grid passes a range THROUGH to every renderer untouched; how it is parsed,
// validated, defaulted, or written to a URL is the consumer's business.

/** The dashboard's active time window. ISO strings (`"2026-07-15"` or full timestamps) — the
 *  package never parses them; renderers hand them to their data client verbatim. */
export interface TimeRange {
  from: string;
  to: string;
}
