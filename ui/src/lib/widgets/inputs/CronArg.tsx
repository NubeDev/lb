// The `cron` arg widget (channel rich responses scope) — an `x-lb-widget:"cron"` arg renders this: the
// shipped visual `CronBuilder` (a lossless 5-field cron round-trip) wrapped to the palette's
// `{ value, onChange }` widget contract. RENDER only; the builder owns the antd-scoped authoring surface
// (FILE-LAYOUT — one widget per file). Reuses the reminders CronBuilder verbatim; no cron logic here.

import { useEffect } from "react";

import { CronBuilder } from "@/features/reminders/CronBuilder";

/** The default schedule shown (and, until edited, COLLECTED) — a daily 9am cron. */
const DEFAULT_CRON = "0 9 * * *";

interface Props {
  /** The current 5-field cron string. */
  value: string;
  /** Called with the new cron string on every edit. */
  onChange: (value: string) => void;
}

export function CronArg({ value, onChange }: Props) {
  // The builder only emits `onChange` on an EDIT, so an unedited cron left the arg EMPTY — `schedule`
  // (a required inline widget) never counted as filled and the rail stuck on it, blocking the whole
  // `/remind` form. Seed the shown default into the value on mount so the default is really collected
  // (WYSIWYG — the builder shows `0 9 * * *`, that is what submit sends unless the user edits it).
  useEffect(() => {
    if (!value) onChange(DEFAULT_CRON);
  }, [value, onChange]);

  return (
    <div className="border-t border-border bg-panel p-2" aria-label="cron picker">
      <label className="mb-1 block text-xs text-muted">Schedule</label>
      <CronBuilder value={value || DEFAULT_CRON} onChange={onChange} />
    </div>
  );
}
