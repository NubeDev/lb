// The panel header's TIME-OVERRIDE badge text. Grafana shows a small header badge whenever a
// panel overrides the dashboard's time range, so a viewer can never mistake a shifted/narrowed
// panel for the dashboard range — that honesty is the whole point of the badge. The HOST
// interprets timeFrom/timeShift when dispatching queries; the grid only announces them.
//
// One responsibility: queryOptions → the badge label (or null when there's nothing to announce).

import type { QueryOptions } from "./dashboard.types";

/** The badge text for a panel's time override, or null when none applies / it's hidden.
 *
 *  - `timeFrom` → the panel's own window (`"6h"` → "Last 6h").
 *  - `timeShift` → the comparison offset (`"1d"` → "1d earlier").
 *  - both → "Last 6h, 1d earlier".
 *  - `relativeTime` is the pre-Grafana vocabulary for the same idea (`"now-6h"`); shown when no
 *    `timeFrom` is set so an existing cell keeps announcing its override.
 *  `hideTimeOverride` → null (the author opted out of the badge, not out of the override). */
export function timeOverrideBadge(qo: QueryOptions | undefined): string | null {
  if (!qo || qo.hideTimeOverride) return null;
  const parts: string[] = [];
  const from = qo.timeFrom || qo.relativeTime;
  if (from) parts.push(`Last ${from.replace(/^now-/, "")}`);
  if (qo.timeShift) parts.push(`${qo.timeShift} earlier`);
  return parts.length > 0 ? parts.join(", ") : null;
}
