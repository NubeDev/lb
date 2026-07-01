// `format.datetime` client verb (user-prefs scope) — render a canonical UTC instant as wall-clock text
// in the viewer's resolved timezone + date/time style. The grant-free utility tier: the HOST owns the
// tz math + DST rules (`chrono-tz`) + the numeric styles, so the client ships NO date logic and no
// timezone database — one implementation for every client (the scope's "thin clients are free").
//
// The instant is epoch MILLISECONDS (the host contract). A source in epoch SECONDS (the flow clock)
// MUST be ×1000'd by the caller before this — see `fieldconfig/format.ts` `dateUnit`. We never guess
// the unit from magnitude (a latent correctness bug for historical / far-future instants).

import type { ResolvedPrefs } from "./prefs.types";
import { invoke } from "@/lib/ipc/invoke";

/** Format `instantMs` (epoch milliseconds, UTC) in `prefs`' timezone + styles. Returns the display
 *  string (e.g. `01/07/2026 14:45`). Mirrors the gateway `POST /format/datetime`. */
export function formatDatetime(instantMs: number, prefs: ResolvedPrefs): Promise<string> {
  return invoke<{ text: string }>("format_datetime", {
    instant: instantMs,
    // Pass the explicit axes (not the whole `prefs` object) so the host formats deterministically from
    // exactly timezone + date_style + time_style — the three the datetime formatter reads.
    timezone: prefs.timezone,
    date_style: prefs.date_style,
    time_style: prefs.time_style,
  }).then((r) => r.text);
}
