// The `cron` arg widget (channel rich responses scope) — an `x-lb-widget:"cron"` arg renders this: the
// shipped visual `CronBuilder` (a lossless 5-field cron round-trip) wrapped to the palette's
// `{ value, onChange }` widget contract. RENDER only; the builder owns the antd-scoped authoring surface
// (FILE-LAYOUT — one widget per file). Reuses the reminders CronBuilder verbatim; no cron logic here.

import { CronBuilder } from "@/features/reminders/CronBuilder";

interface Props {
  /** The current 5-field cron string. */
  value: string;
  /** Called with the new cron string on every edit. */
  onChange: (value: string) => void;
}

export function CronArg({ value, onChange }: Props) {
  return (
    <div className="border-t border-border bg-panel p-2" aria-label="cron picker">
      <label className="mb-1 block text-xs text-muted">Schedule</label>
      <CronBuilder value={value || "0 9 * * *"} onChange={onChange} />
    </div>
  );
}
