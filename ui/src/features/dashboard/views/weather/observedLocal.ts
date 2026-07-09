// Render the weather observation instant (a UTC epoch in SECONDS, per the `weather.current` contract)
// as wall-clock text in the VIEWER's own browser timezone — so a Da Nang viewer sees 17:45, not the
// raw 10:45Z. Pure: `Date` + `toLocaleString` do the tz + DST conversion from the browser locale.
// One responsibility: epoch seconds → local "YYYY-MM-DD HH:MM" string.

/** Format `epochSeconds` (UTC) in the browser's local timezone as `YYYY-MM-DD HH:MM`. Returns "" for a
 *  non-finite input (an absent/garbled timestamp) so the panel renders nothing rather than "Invalid Date". */
export function observedLocal(epochSeconds: number | null): string {
  if (epochSeconds == null || !Number.isFinite(epochSeconds)) return "";
  const d = new Date(epochSeconds * 1000);
  // Locale-independent Y-M-D H:M in LOCAL time (each getter is the viewer's timezone).
  const pad = (n: number) => String(n).padStart(2, "0");
  return (
    `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ` +
    `${pad(d.getHours())}:${pad(d.getMinutes())}`
  );
}
