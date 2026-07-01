// A memoizing wrapper over `format.datetime` (user-prefs / flow-ts-display scope). A dashboard renders
// the SAME instant many times (every refresh tick, every re-render); the resolved wall-clock text for a
// given (instant, tz, date_style, time_style) never changes, so we cache it. First render is one host
// round-trip; every render after is synchronous from the cache. This is what keeps a per-value async
// formatter cheap without a formatter call per frame.

import { formatDatetime } from "./formatDatetime";
import type { ResolvedPrefs } from "./prefs.types";

/** key = `instantMs|tz|date_style|time_style` → the settled display string. Bounded implicitly by the
 *  small set of distinct (instant, prefs) a dashboard shows; a flow value changes at canvas cadence so
 *  the instant set is naturally small (last-value per node). */
const cache = new Map<string, string>();
/** In-flight promises so concurrent renders of the same key share one round-trip. */
const inflight = new Map<string, Promise<string>>();

function keyOf(instantMs: number, prefs: ResolvedPrefs): string {
  return `${instantMs}|${prefs.timezone}|${prefs.date_style}|${prefs.time_style}`;
}

/** The cached display string if already resolved for this (instant, prefs), else `undefined`. Synchronous
 *  — a renderer calls this first and shows the value immediately on a cache hit. */
export function cachedDatetime(instantMs: number, prefs: ResolvedPrefs): string | undefined {
  return cache.get(keyOf(instantMs, prefs));
}

/** Resolve + cache the display string for this (instant, prefs). Concurrent callers share one call. */
export function formatDatetimeCached(instantMs: number, prefs: ResolvedPrefs): Promise<string> {
  const key = keyOf(instantMs, prefs);
  const hit = cache.get(key);
  if (hit !== undefined) return Promise.resolve(hit);
  const pending = inflight.get(key);
  if (pending) return pending;
  const p = formatDatetime(instantMs, prefs)
    .then((text) => {
      cache.set(key, text);
      inflight.delete(key);
      return text;
    })
    .catch((e) => {
      inflight.delete(key);
      throw e;
    });
  inflight.set(key, p);
  return p;
}
