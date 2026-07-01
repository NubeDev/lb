// `useFormattedValue` — the React seam a panel uses to turn a canonical value + `fieldConfig` options
// into a display string, INCLUDING the async datetime path (viz field-config + flow-ts-display scope).
//
// `formatValue` (pure, sync) handles numbers/quantities/plain values and returns a synchronous fallback
// for a datetime field. This hook adds the one async case: a `datetime` unit is rendered through
// `format.datetime` in the VIEWER's resolved prefs (the host owns tz/DST/style — no client date math).
// The value is normalized to epoch ms per the unit's `dateUnit` (`s` for the flow clock, `ms` default),
// resolved once and cached, so re-renders are synchronous after the first. Until the async render
// settles (or if prefs/resolve is unavailable) it shows the sync fallback — never a wrong localized
// value, never a blank.
//
// One responsibility: pick sync-vs-async formatting for one value. The pure math stays in `format.ts`;
// the prefs plumbing stays in `lib/prefs`. This composes them for a renderer.

import { useEffect, useState } from "react";

import type { FieldOptions } from "@/lib/dashboard";
import { formatValue } from "./format";
import { resolveUnit } from "./units";
import { useResolvedPrefs } from "@/lib/prefs/useResolvedPrefs";
import { cachedDatetime, formatDatetimeCached } from "@/lib/prefs/formatDatetimeCached";

/** Normalize a canonical epoch value to milliseconds per the field's declared `dateUnit`. Seconds
 *  (the flow clock) × 1000; ms unchanged. Declared, never magnitude-guessed. */
function toEpochMs(value: number, dateUnit: "s" | "ms" | undefined): number {
  return dateUnit === "s" ? value * 1000 : value;
}

/** The display string for `value` under `opts`. A `datetime` field resolves to the viewer's wall-clock
 *  via the prefs formatter (async, cached); every other field is the synchronous `formatValue`. */
export function useFormattedValue(value: unknown, opts: FieldOptions | undefined): string {
  const prefs = useResolvedPrefs();
  const mapping = resolveUnit(opts?.unit);
  const isDatetime = mapping.kind === "datetime" && typeof value === "number" && Number.isFinite(value);
  const instantMs = isDatetime ? toEpochMs(value as number, mapping.dateUnit) : null;

  // Seed from the cache (synchronous hit on a re-render / a value already seen this session).
  const [text, setText] = useState<string | null>(() =>
    instantMs !== null && prefs ? cachedDatetime(instantMs, prefs) ?? null : null,
  );

  useEffect(() => {
    if (instantMs === null || !prefs) {
      setText(null);
      return;
    }
    let cancelled = false;
    formatDatetimeCached(instantMs, prefs)
      .then((t) => {
        if (!cancelled) setText(t);
      })
      .catch(() => {
        if (!cancelled) setText(null); // fall back to the sync render below
      });
    return () => {
      cancelled = true;
    };
  }, [instantMs, prefs]);

  // The prefs-resolved wall-clock when available; otherwise the sync fallback (canonical + unit /
  // ISO-ish for a not-yet-resolved datetime) — honest, never wrong.
  if (isDatetime && text !== null) return text;
  return formatValue(value, opts).text;
}
