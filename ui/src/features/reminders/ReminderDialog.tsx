// The reminder author/edit dialog — the full-CRUD authoring surface for one reminder (reminders
// scope). Create and edit share ONE dialog because `reminder.create` upserts by id: a new reminder
// is a blank draft; editing seeds the draft from the record and locks the id (re-authoring the same
// id is the update path). The schedule is point-and-click via `CronBuilder` (the human never types
// cron); the action is picked + configured in `ActionEditor`. `maxRuns` (run cap) and the schedule
// ride the same `reminder.create`/`reminder.update` verbs the Rust deny-tests prove. One component,
// one concern (FILE-LAYOUT): gather a valid draft and hand it to the caller's create/update verb.

import { useState } from "react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { CronBuilder } from "./CronBuilder";
import { ActionEditor } from "./ActionEditor";
import type { Reminder, ReminderAction } from "@/lib/reminders/reminders.types";

const BLANK_ACTION: ReminderAction = { kind: "channel-post", channel: "", body: "" };

export interface ReminderDraft {
  id: string;
  schedule: string;
  maxRuns: number | null;
  action: ReminderAction;
}

interface Props {
  /** The reminder being edited, or `null` for a fresh create. */
  editing: Reminder | null;
  /** Persist the draft — `create` for a new one, `update` for an edit (the caller wires the verb). */
  onSubmit: (draft: ReminderDraft) => Promise<void>;
  onClose: () => void;
}

export function ReminderDialog({ editing, onSubmit, onClose }: Props) {
  const isEdit = editing !== null;

  const [id, setId] = useState(editing?.id ?? "");
  const [schedule, setSchedule] = useState(editing?.schedule ?? "0 8 * * 0,1");
  const [maxRuns, setMaxRuns] = useState(editing?.maxRuns != null ? String(editing.maxRuns) : "");
  const [action, setAction] = useState<ReminderAction>(editing?.action ?? BLANK_ACTION);
  const [busy, setBusy] = useState(false);

  const canSubmit = id.trim() !== "" && schedule.trim() !== "" && !busy;

  async function submit() {
    if (!canSubmit) return;
    setBusy(true);
    try {
      const cap = maxRuns.trim() === "" ? null : Number(maxRuns);
      await onSubmit({ id: id.trim(), schedule, maxRuns: cap, action });
      onClose();
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog open onOpenChange={(o) => (o ? undefined : onClose())}>
      <DialogContent className="max-w-lg gap-4">
        <DialogHeader>
          <DialogTitle>{isEdit ? `Edit ${editing.id}` : "New reminder"}</DialogTitle>
          <DialogDescription>
            A durable, workspace-scoped schedule that fires one action when it comes due.
          </DialogDescription>
        </DialogHeader>

        <div className="max-h-[60vh] space-y-4 overflow-y-auto pr-1">
          <div className="space-y-1">
            <label htmlFor="reminder-id" className="text-xs font-medium text-muted">
              Name
            </label>
            <Input
              id="reminder-id"
              aria-label="reminder id"
              placeholder="standup-ping"
              value={id}
              disabled={isEdit}
              onChange={(e) => setId(e.target.value)}
            />
            {isEdit && <p className="text-[11px] text-muted">The name is the key and can't change.</p>}
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
        </div>

        <DialogFooter>
          <Button type="button" variant="outline" onClick={onClose}>
            Cancel
          </Button>
          <Button
            type="button"
            aria-label={isEdit ? "save reminder" : "create reminder"}
            disabled={!canSubmit}
            onClick={() => void submit()}
          >
            {isEdit ? "Save changes" : "Create reminder"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
