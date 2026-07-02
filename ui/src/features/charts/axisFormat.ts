// X-axis tick formatting for the shared chart — turns a raw x value (an ISO string, an epoch number,
// or a plain category) into a compact, human tick. Temporal x values become a short time/date so a
// timeseries reads at a glance instead of showing raw ISO strings or epoch integers. Kept separate so
// `PlotChart` stays a pure composition of chart elements.
//
// One responsibility: x value → tick label + whether the axis is temporal.

const ISO_DATE = /^\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}/;
const EPOCH_S_MIN = 1_000_000_000;
const EPOCH_MS_MAX = 4_102_444_800_000;

function toDate(value: unknown): Date | null {
  if (value instanceof Date) return value;
  if (typeof value === "number" && Number.isInteger(value) && value >= EPOCH_S_MIN && value <= EPOCH_MS_MAX) {
    return new Date(value < 1e12 ? value * 1000 : value);
  }
  if (typeof value === "string" && ISO_DATE.test(value)) {
    const d = new Date(value);
    return Number.isNaN(d.getTime()) ? null : d;
  }
  return null;
}

/** True when the sampled x values look temporal (so the axis formats as time). */
export function isTemporalAxis(sample: unknown): boolean {
  return toDate(sample) !== null;
}

/** Format one x tick: a short local time for a temporal value, else the value as a trimmed string. */
export function formatXTick(value: unknown): string {
  const d = toDate(value);
  if (d) {
    // Same-day series show HH:MM; longer spans show MM-DD HH:MM. Cheap heuristic: always show time,
    // prefix the date only when it isn't today.
    const now = new Date();
    const sameDay =
      d.getFullYear() === now.getFullYear() && d.getMonth() === now.getMonth() && d.getDate() === now.getDate();
    const time = d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
    if (sameDay) return time;
    const date = d.toLocaleDateString(undefined, { month: "short", day: "numeric" });
    return `${date} ${time}`;
  }
  const s = String(value ?? "");
  return s.length > 16 ? `${s.slice(0, 15)}…` : s;
}
