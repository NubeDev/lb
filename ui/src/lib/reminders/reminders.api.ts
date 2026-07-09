// The reminders API client — one call per export, mirroring the Rust `reminder.*` verbs and the
// host MCP bridge 1:1 (reminders scope). The UI never calls `invoke` directly; it goes through these
// named verbs (FILE-LAYOUT frontend rules). Each rides the host-mediated `POST /mcp/call` bridge
// (`mcp_call`), so the workspace + principal come from the session token (the hard wall, §7) and
// each verb re-checks `mcp:reminder.<verb>:call` server-side.

import type { Reminder, ReminderAction } from "./reminders.types";
import { invoke } from "@/lib/ipc/invoke";

/** The wire shape the host `reminder_json` emits (snake_case → camelCase happens server-side). */
interface CreateArgs {
  id: string;
  schedule: string;
  max_runs?: number | null;
  action: ReminderAction;
  ts: number;
}

/** Create a reminder. Mirrors `reminder.create`. Returns the persisted reminder (nextAttemptTs set). */
export function createReminder(
  id: string,
  schedule: string,
  action: ReminderAction,
  opts?: { maxRuns?: number | null; ts?: number },
): Promise<Reminder> {
  const args: CreateArgs = {
    id,
    schedule,
    action,
    ts: opts?.ts ?? 0,
  };
  if (opts?.maxRuns !== undefined) args.max_runs = opts.maxRuns;
  return invoke<Reminder>("mcp_call", { tool: "reminder.create", args });
}

/** Update a reminder (pause/resume `enabled`, reschedule, change the action). Mirrors `reminder.update`. */
export function updateReminder(
  id: string,
  patch: {
    schedule?: string;
    maxRuns?: number | null;
    enabled?: boolean;
    action?: ReminderAction;
    ts?: number;
  },
): Promise<Reminder> {
  const args: Record<string, unknown> = { id, ts: patch.ts ?? 0 };
  if (patch.schedule !== undefined) args.schedule = patch.schedule;
  if (patch.maxRuns !== undefined) args.max_runs = patch.maxRuns;
  if (patch.enabled !== undefined) args.enabled = patch.enabled;
  if (patch.action !== undefined) args.action = patch.action;
  return invoke<Reminder>("mcp_call", { tool: "reminder.update", args });
}

/** Soft-delete a reminder (idempotent tombstone). Mirrors `reminder.delete`. */
export function deleteReminder(id: string, ts?: number): Promise<{ ok: true }> {
  return invoke<{ ok: true }>("mcp_call", { tool: "reminder.delete", args: { id, ts: ts ?? 0 } });
}

/** Read a reminder by id (`null` if absent/deleted). Mirrors `reminder.get`. */
export function getReminder(id: string): Promise<Reminder | null> {
  return invoke<{ reminder: Reminder | null }>("mcp_call", {
    tool: "reminder.get",
    args: { id },
  }).then((r) => r.reminder);
}

/** Fire a reminder now (run-now), independent of its schedule. Mirrors `reminder.fire`. Returns
 *  whether a fresh firing was enqueued (`false` if already fired at this instant — idempotent).
 *  Note: run-now re-resolves the stored principal's caps at fire time; a dev-login may be denied
 *  (a documented pre-existing limitation) even though scheduled firing works under a durable grant. */
export function fireReminder(id: string, ts?: number): Promise<{ fired: boolean; scheduledTs?: number }> {
  return invoke<{ fired: boolean; scheduled_ts?: number }>("mcp_call", {
    tool: "reminder.fire",
    args: { id, ts: ts ?? 0 },
  }).then((r) => ({ fired: r.fired, scheduledTs: r.scheduled_ts }));
}

/** List every non-deleted reminder in the workspace. Mirrors `reminder.list`. */
export function listReminders(): Promise<Reminder[]> {
  return invoke<{ reminders: Reminder[] }>("mcp_call", {
    tool: "reminder.list",
    args: {},
  }).then((r) => r.reminders);
}
