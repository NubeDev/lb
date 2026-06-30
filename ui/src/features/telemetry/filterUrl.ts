// URL-encode the telemetry filter so a console view is shareable/deep-linkable (telemetry-console
// scope: "filters URL-encoded so a view is shareable"). One responsibility: the codec between the
// `TelemetryFilter` and a `URLSearchParams` string. Empty fields are omitted (a clean, short link).

import type { TelemetryFilter, TelemetryLevel, TelemetryOutcome } from "@/lib/telemetry";

const LEVELS: TelemetryLevel[] = ["error", "warn", "info", "debug", "trace"];
const OUTCOMES: TelemetryOutcome[] = ["allow", "deny", "error"];

/** Encode a filter to a query string (no leading `?`). Stable key order for predictable links. */
export function encodeFilterToQuery(filter: TelemetryFilter): string {
  const p = new URLSearchParams();
  if (filter.source) p.set("source", filter.source);
  if (filter.actor) p.set("actor", filter.actor);
  if (filter.level) p.set("level", filter.level);
  if (filter.outcome) p.set("outcome", filter.outcome);
  if (filter.traceId) p.set("trace", filter.traceId);
  if (filter.text) p.set("q", filter.text);
  if (filter.since != null) p.set("since", String(filter.since));
  if (filter.until != null) p.set("until", String(filter.until));
  return p.toString();
}

/** Decode a query string (or `URLSearchParams`) back to a filter. Unknown level/outcome values are
 *  dropped (the filter set is bounded), so a hand-edited link never injects an invalid clause. */
export function decodeFilterFromQuery(
  query: string | URLSearchParams,
): TelemetryFilter {
  const p = typeof query === "string" ? new URLSearchParams(query) : query;
  const filter: TelemetryFilter = {};
  const source = p.get("source");
  if (source) filter.source = source;
  const actor = p.get("actor");
  if (actor) filter.actor = actor;
  const level = p.get("level");
  if (level && (LEVELS as string[]).includes(level)) filter.level = level as TelemetryLevel;
  const outcome = p.get("outcome");
  if (outcome && (OUTCOMES as string[]).includes(outcome)) {
    filter.outcome = outcome as TelemetryOutcome;
  }
  const trace = p.get("trace");
  if (trace) filter.traceId = trace;
  const text = p.get("q");
  if (text) filter.text = text;
  const since = p.get("since");
  if (since && Number.isFinite(Number(since))) filter.since = Number(since);
  const until = p.get("until");
  if (until && Number.isFinite(Number(until))) filter.until = Number(until);
  return filter;
}
