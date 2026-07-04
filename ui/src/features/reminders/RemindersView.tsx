// The reminders view — author a durable, workspace-scoped schedule that fires ONE action when due
// (reminders scope). Layout + wiring only; data lives in `useReminders` (FILE-LAYOUT). The schedule
// is point-and-click via the `react-js-cron` builder (the `CronBuilder` wrapper) — the human never
// types cron; it round-trips the 5-field string the host stores. The action is picked + configured
// in `ActionEditor` (channel post / MCP tool / outbox). `maxRuns` (the run cap) and `enabled`
// (pause/resume) ride the same `reminder.create`/`reminder.update` verbs the Rust deny-tests prove.
//
// Every verb rides the host-mediated `POST /mcp/call` bridge — the gateway re-checks
// `mcp:reminder.<verb>:call` server-side, so this page is exactly as denied as a forged call.

import { useState } from "react";
import { CalendarClock } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useReminders } from "./useReminders";
import { CronBuilder } from "./CronBuilder";
import { ActionEditor } from "./ActionEditor";
import type { ReminderAction } from "@/lib/reminders/reminders.types";

interface Props {
  ws: string;
}

const BLANK_ACTION: ReminderAction = { kind: "channel-post", channel: "", body: "" };

export function RemindersView({ ws }: Props) {
  const { reminders, loading, error, create, update, remove } = useReminders();

  // The draft a new reminder is authored from. `id` is the workspace-unique key (re-create upserts).
  const [id, setId] = useState("");
  const [schedule, setSchedule] = useState("0 8 * * 0,1");
  const [maxRuns, setMaxRuns] = useState("");
  const [action, setAction] = useState<ReminderAction>(BLANK_ACTION);
  const [busy, setBusy] = useState(false);

  const canCreate = id.trim() !== "" && schedule.trim() !== "" && !busy;

  async function onCreate() {
    if (!canCreate) return;
    setBusy(true);
    try {
      const cap = maxRuns.trim() === "" ? null : Number(maxRuns);
      await create(id.trim(), schedule, action, { maxRuns: cap });
      // Reset the draft for the next one (keep the schedule — most users author a batch on one cron).
      setId("");
      setMaxRuns("");
      setAction(BLANK_ACTION);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="flex h-full flex-col bg-bg">
      <header className="flex items-center gap-2 border-b border-border px-4 py-3">
        <CalendarClock size={16} className="text-muted" />
        <h1 className="text-sm font-medium">Reminders</h1>
        <span className="ml-auto text-xs text-muted">{ws}</span>
      </header>

      {error && (
        <div role="alert" className="bg-panel px-4 py-2 text-xs text-accent">
          {error === "denied" ? "You don't have access to reminders." : error}
        </div>
      )}

      <div className="flex min-h-0 flex-1">
        {/* Author rail — the create form. */}
        <div
          aria-label="reminder author"
          className="w-96 shrink-0 space-y-3 overflow-y-auto border-r border-border px-4 py-4"
        >
          <div className="space-y-1">
            <label htmlFor="reminder-id" className="text-xs font-medium text-muted">
              Name
            </label>
            <Input
              id="reminder-id"
              aria-label="reminder id"
              placeholder="standup-ping"
              value={id}
              onChange={(e) => setId(e.target.value)}
            />
          </div>

          <div className="space-y-1">
            <span className="text-xs font-medium text-muted">Schedule</span>
            <CronBuilder value={schedule} onChange={setSchedule} />
          </div>

          <div className="space-y-1">
            <label htmlFor="reminder-maxruns" className="text-xs font-medium text-muted">
              Run cap (blank = forever)
            </label>
            <Input
              id="reminder-maxruns"
              aria-label="reminder max runs"
              type="number"
              min={1}
              placeholder="∞"
              value={maxRuns}
              onChange={(e) => setMaxRuns(e.target.value)}
            />
          </div>

          <ActionEditor action={action} onChange={setAction} />

          <Button
            type="button"
            aria-label="create reminder"
            disabled={!canCreate}
            onClick={() => void onCreate()}
            className="w-full"
          >
            Create reminder
          </Button>
        </div>

        {/* List — the workspace's reminders, each with pause/resume + delete. */}
        <div aria-label="reminder list" className="min-w-0 flex-1 overflow-y-auto px-4 py-4">
          {loading ? (
            <div className="text-sm text-muted">Loading…</div>
          ) : reminders.length === 0 ? (
            <div className="flex h-full items-center justify-center text-sm text-muted">
              No reminders yet — author one on the left.
            </div>
          ) : (
            <ul className="space-y-2">
              {reminders.map((r) => (
                <li
                  key={r.id}
                  aria-label={`reminder ${r.id}`}
                  className="rounded-md border border-border bg-card px-3 py-2 text-sm"
                >
                  <div className="flex items-center gap-2">
                    <span className="font-medium">{r.id}</span>
                    <code className="rounded-md bg-panel px-1 text-xs text-muted">{r.schedule}</code>
                    <span
                      aria-label={`reminder ${r.id} status`}
                      className="ml-auto text-xs text-muted"
                    >
                      {r.enabled ? "enabled" : "paused"} · {r.status}
                      {r.maxRuns != null ? ` · ${r.runs}/${r.maxRuns}` : ""}
                    </span>
                  </div>
                  <div className="mt-1 text-xs text-muted">
                    {r.action.kind === "channel-post" && `post → #${r.action.channel}`}
                    {r.action.kind === "mcp-tool" && `call → ${r.action.tool}`}
                    {r.action.kind === "outbox" && `effect → ${r.action.target}/${r.action.action}`}
                  </div>
                  <div className="mt-2 flex gap-2">
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      aria-label={`toggle reminder ${r.id}`}
                      onClick={() => void update(r.id, { enabled: !r.enabled })}
                    >
                      {r.enabled ? "Pause" : "Resume"}
                    </Button>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      aria-label={`delete reminder ${r.id}`}
                      onClick={() => void remove(r.id)}
                    >
                      Delete
                    </Button>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
    </section>
  );
}
