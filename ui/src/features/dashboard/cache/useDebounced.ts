// Debounce a value (dashboard-query-cache-scope). The shared `useVizQuery` consolidates the three
// per-instance debounces the editor used to run into ONE debounce on the KEY INPUT: the resolved spec is
// debounced here, so a burst of keystrokes settles to a single new query key (and thus one `viz.query`),
// not one per keystroke or one per consumer (scope: "Debounce moves" risk). One responsibility: hold a
// value, then release it after `ms` of quiet.

import { useEffect, useState } from "react";

/** Return `value` delayed by `ms` of quiet — the returned value only updates once `value` has been stable
 *  for `ms`. The FIRST value passes through on mount (nothing to wait for); each change restarts the timer. */
export function useDebounced<T>(value: T, ms: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const timer = setTimeout(() => setDebounced(value), ms);
    return () => clearTimeout(timer);
  }, [value, ms]);
  return debounced;
}
