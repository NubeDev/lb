// The reminders stat row — a compact KPI strip derived ENTIRELY from the workspace's reminder
// records (`reminder.list`), never a fabricated history feed. There is no `reminder.history` verb at
// v1 (each firing is an internal `reminder-fire` lb-job with no list surface — reminders scope), so
// the honest stats we can show are the record-level facts: how many reminders exist, how many are
// live vs paused vs exhausted, the total firings so far (the summed `runs` counter each firing
// advances), and when the next one is due. One component, one concern (FILE-LAYOUT): read the list,
// render the tiles.

import type { LucideIcon } from "lucide-react";
import { CalendarClock, CheckCircle2, PauseCircle, Play, Zap } from "lucide-react";

import type { Reminder } from "@/lib/reminders/reminders.types";

interface Props {
  reminders: Reminder[];
}

interface Tile {
  label: string;
  value: string;
  hint?: string;
  icon: LucideIcon;
  tone: "fg" | "success" | "muted" | "accent";
}

const TONE: Record<Tile["tone"], string> = {
  fg: "text-fg",
  success: "text-success",
  muted: "text-muted",
  accent: "text-accent",
};

/** The host `nextAttemptTs` is a LOGICAL clock in seconds; `0` = unscheduled (paused/done). Render
 *  the soonest future firing across all active reminders as a friendly relative string. */
function nextFireHint(reminders: Reminder[]): string {
  const nowSecs = Math.floor(Date.now() / 1000);
  const upcoming = reminders
    .filter((r) => r.enabled && r.status === "active" && r.nextAttemptTs > 0)
    .map((r) => r.nextAttemptTs)
    .filter((ts) => ts >= nowSecs)
    .sort((a, b) => a - b);
  if (upcoming.length === 0) return "—";
  const delta = upcoming[0] - nowSecs;
  if (delta < 60) return "under a minute";
  if (delta < 3600) return `in ${Math.round(delta / 60)} min`;
  if (delta < 86400) return `in ${Math.round(delta / 3600)} h`;
  return `in ${Math.round(delta / 86400)} d`;
}

export function RemindersStats({ reminders }: Props) {
  const active = reminders.filter((r) => r.enabled && r.status === "active").length;
  const paused = reminders.filter((r) => !r.enabled && r.status === "active").length;
  const done = reminders.filter((r) => r.status === "done").length;
  const totalRuns = reminders.reduce((sum, r) => sum + r.runs, 0);

  const tiles: Tile[] = [
    { label: "Reminders", value: String(reminders.length), icon: CalendarClock, tone: "fg" },
    { label: "Active", value: String(active), icon: Play, tone: "success" },
    { label: "Paused", value: String(paused), icon: PauseCircle, tone: "muted" },
    { label: "Completed", value: String(done), icon: CheckCircle2, tone: "muted" },
    { label: "Total firings", value: String(totalRuns), hint: "runs recorded", icon: Zap, tone: "accent" },
    { label: "Next firing", value: nextFireHint(reminders), icon: CalendarClock, tone: "fg" },
  ];

  return (
    <div
      aria-label="reminder stats"
      className="grid grid-cols-2 gap-2 border-b border-border px-4 py-3 sm:grid-cols-3 lg:grid-cols-6"
    >
      {tiles.map((t) => (
        <div key={t.label} className="rounded-md border border-border bg-card px-3 py-2">
          <div className="flex items-center gap-1.5 text-xs text-muted">
            <t.icon size={13} className={TONE[t.tone]} strokeWidth={1.75} />
            <span>{t.label}</span>
          </div>
          <div className={`mt-1 text-lg font-semibold tabular-nums ${TONE[t.tone]}`}>{t.value}</div>
          {t.hint && <div className="text-[11px] text-muted">{t.hint}</div>}
        </div>
      ))}
    </div>
  );
}
