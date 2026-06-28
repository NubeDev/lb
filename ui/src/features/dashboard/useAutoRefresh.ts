// The auto-refresh tick (widget-config-vars Slice 4). Given the URL refresh interval, returns a
// `refreshKey` that increments every interval — threaded into `useVarScope`/`useSource`/the variable bar
// so a tick re-resolves query variables + re-runs each cell's read source. Pauses when the tab is hidden
// (no work for an unseen dashboard); resumes on visibility. `off`/absent = a frozen key (no ticking).
//
// In-flight dedupe is the source hooks' concern: each re-keyed effect cancels its prior run, so a slow
// read doesn't stack. This hook only owns the cadence. One hook per file.

import { useEffect, useRef, useState } from "react";

/** Parse a refresh interval (`5s`/`30s`/`1m`/`5m`/`15m`) to milliseconds; `0` for off/absent/unknown. */
export function refreshMs(interval: string | undefined): number {
  if (!interval) return 0;
  const m = /^(\d+)(s|m)$/.exec(interval.trim());
  if (!m) return 0;
  const n = Number(m[1]);
  return m[2] === "m" ? n * 60_000 : n * 1000;
}

/** A monotonically-increasing key that bumps every `interval`. Pauses while the tab is hidden. */
export function useAutoRefresh(interval: string | undefined): number {
  const [key, setKey] = useState(0);
  const ms = refreshMs(interval);
  const timer = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    if (ms <= 0) return; // off — no ticking

    const stop = () => {
      if (timer.current) {
        clearInterval(timer.current);
        timer.current = null;
      }
    };
    const start = () => {
      if (timer.current) return; // already running (dedupe the interval)
      timer.current = setInterval(() => setKey((k) => k + 1), ms);
    };

    // Pause when hidden, resume when visible — no work for an unseen dashboard (thundering-herd guard).
    const onVisibility = () => {
      if (typeof document !== "undefined" && document.hidden) stop();
      else start();
    };

    onVisibility();
    if (typeof document !== "undefined") {
      document.addEventListener("visibilitychange", onVisibility);
    }
    return () => {
      stop();
      if (typeof document !== "undefined") {
        document.removeEventListener("visibilitychange", onVisibility);
      }
    };
  }, [ms]);

  return key;
}
