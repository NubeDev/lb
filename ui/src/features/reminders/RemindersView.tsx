// The reminders view — the workspace's durable, scheduled-action table (reminders scope). A stat
// strip up top (record-derived KPIs — `RemindersStats`), a "New reminder" toolbar, and a shadcn
// `Table` of every reminder with per-row CRUD: Run now (`reminder.fire`), Pause/Resume + Edit
// (`reminder.update`/upsert via the dialog), and Delete (`reminder.delete`, gated behind
// `ConfirmDestructive`). Layout + wiring only; data lives in `useReminders` (FILE-LAYOUT). The
// authoring surface (visual cron builder + action editor) is the `ReminderDialog`; the human never
// types cron.
//
// Every verb rides the host-mediated `POST /mcp/call` bridge — the gateway re-checks
// `mcp:reminder.<verb>:call` server-side, so this page is exactly as denied as a forged call.

import { useState } from "react";
import { CalendarClock, Pencil, Play, Plus } from "lucide-react";

import { AppEmptyState } from "@/components/app/empty-state";
import { AppPageHeader } from "@/components/app/page-header";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { ConfirmDestructive } from "@/features/confirm";
import { useReminders } from "./useReminders";
import { RemindersStats } from "./RemindersStats";
import { ReminderDialog, type ReminderDraft } from "./ReminderDialog";
import type { Reminder } from "@/lib/reminders/reminders.types";

interface Props {
  ws: string;
}

/** A one-line summary of a reminder's action, for the table's Action column. */
function actionSummary(r: Reminder): string {
  switch (r.action.kind) {
    case "channel-post":
      return `post → #${r.action.channel}`;
    case "mcp-tool":
      return `call → ${r.action.tool}`;
    case "outbox":
      return `effect → ${r.action.target}/${r.action.action}`;
  }
}

/** The dialog's open state: closed, a fresh create, or editing a specific record. */
type Editor = { open: false } | { open: true; editing: Reminder | null };

export function RemindersView({ ws }: Props) {
  const { reminders, loading, error, create, update, remove, fire } = useReminders();

  const [editor, setEditor] = useState<Editor>({ open: false });
  const [pendingDelete, setPendingDelete] = useState<Reminder | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  async function onSubmit(draft: ReminderDraft) {
    // create() upserts by id — the same verb path serves both a new reminder and an edit.
    await create(draft.id, draft.schedule, draft.action, { maxRuns: draft.maxRuns });
  }

  async function onRunNow(r: Reminder) {
    setNotice(null);
    try {
      const res = await fire(r.id);
      setNotice(res.fired ? `Fired “${r.id}” now.` : `“${r.id}” already fired at this instant.`);
    } catch (e) {
      // Run-now re-resolves the stored principal's caps at fire time — a dev-login is denied here
      // (a documented pre-existing limitation). Surface it inline, never as a page-level failure.
      const msg = e instanceof Error ? e.message : String(e);
      setNotice(/denied/i.test(msg) ? `Run-now for “${r.id}” was denied (fire cap not granted).` : msg);
    }
  }

  return (
    <section className="flex h-full min-w-0 flex-col bg-bg text-fg">
      <AppPageHeader
        icon={CalendarClock}
        title="Reminders"
        description="Durable, workspace-scoped schedules that fire one action when due."
        workspace={ws}
        actions={
          <Button
            type="button"
            size="sm"
            aria-label="new reminder"
            onClick={() => setEditor({ open: true, editing: null })}
          >
            <Plus size={14} />
            New reminder
          </Button>
        }
      />

      {error && (
        <Alert variant="destructive" className="mx-4 mt-3">
          <AlertDescription>
            {error === "denied" ? "You don't have access to reminders." : error}
          </AlertDescription>
        </Alert>
      )}
      {notice && (
        <Alert role="status" className="mx-4 mt-3">
          <AlertDescription>{notice}</AlertDescription>
        </Alert>
      )}

      <RemindersStats reminders={reminders} />

      <div aria-label="reminder list" className="min-h-0 flex-1 overflow-y-auto">
        {loading ? (
          <div className="p-4 text-sm text-muted">Loading…</div>
        ) : reminders.length === 0 ? (
          <AppEmptyState
            icon={CalendarClock}
            title="No reminders yet"
            description="Schedule an action to fire later — once, N times, or recurring. Start with “New reminder”."
          />
        ) : (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Name</TableHead>
                <TableHead>Schedule</TableHead>
                <TableHead>Action</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Runs</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {reminders.map((r) => (
                <TableRow key={r.id} aria-label={`reminder ${r.id}`}>
                  <TableCell className="font-medium text-fg">{r.id}</TableCell>
                  <TableCell>
                    <code className="rounded bg-panel px-1 text-[11px] text-muted">{r.schedule}</code>
                  </TableCell>
                  <TableCell className="text-muted">{actionSummary(r)}</TableCell>
                  <TableCell>
                    <span aria-label={`reminder ${r.id} status`} className="flex items-center gap-1.5">
                      {r.status === "done" ? (
                        <Badge variant="secondary">done</Badge>
                      ) : r.enabled ? (
                        <Badge variant="success">enabled</Badge>
                      ) : (
                        <Badge variant="warning">paused</Badge>
                      )}
                    </span>
                  </TableCell>
                  <TableCell className="tabular-nums text-muted">
                    {r.runs}
                    {r.maxRuns != null ? ` / ${r.maxRuns}` : ""}
                  </TableCell>
                  <TableCell>
                    <div className="flex items-center justify-end gap-1">
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        aria-label={`run reminder ${r.id} now`}
                        title="Run now"
                        onClick={() => void onRunNow(r)}
                      >
                        <Play size={14} />
                      </Button>
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        aria-label={`toggle reminder ${r.id}`}
                        disabled={r.status === "done"}
                        onClick={() => void update(r.id, { enabled: !r.enabled })}
                      >
                        {r.enabled ? "Pause" : "Resume"}
                      </Button>
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        aria-label={`edit reminder ${r.id}`}
                        title="Edit"
                        onClick={() => setEditor({ open: true, editing: r })}
                      >
                        <Pencil size={14} />
                      </Button>
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        aria-label={`delete reminder ${r.id}`}
                        onClick={() => setPendingDelete(r)}
                      >
                        Delete
                      </Button>
                    </div>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
      </div>

      {editor.open && (
        <ReminderDialog
          editing={editor.editing}
          onSubmit={onSubmit}
          onClose={() => setEditor({ open: false })}
        />
      )}

      {pendingDelete && (
        <ConfirmDestructive
          title={`Delete ${pendingDelete.id}`}
          consequence="The reminder stops firing and is removed from the workspace. This can't be undone."
          reversible={false}
          escalation="none"
          confirmLabel="Delete reminder"
          onConfirm={() => {
            const id = pendingDelete.id;
            setPendingDelete(null);
            void remove(id);
          }}
          onCancel={() => setPendingDelete(null)}
        />
      )}
    </section>
  );
}
