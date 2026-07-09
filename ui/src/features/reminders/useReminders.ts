// The data hook for the reminders view — fetches the list and exposes create/update/delete that
// refresh it. One hook per the FILE-LAYOUT frontend rules (a hook is the verb). All verbs go through
// the `reminder.*` API client (the `POST /mcp/call` bridge); the gateway re-checks every cap.

import { useCallback, useEffect, useState } from "react";

import {
  createReminder,
  deleteReminder,
  fireReminder,
  listReminders,
  updateReminder,
} from "@/lib/reminders/reminders.api";
import type { Reminder, ReminderAction } from "@/lib/reminders/reminders.types";

interface State {
  reminders: Reminder[];
  loading: boolean;
  error: string | null;
}

/** The host's `ts` is a LOGICAL clock in **seconds** since the epoch (the same unit `next_after`
 *  feeds croner — see `lb_reminders::model`/`next_after`). `Date.now()` is milliseconds; passing it
 *  raw makes the host compute the next cron slot from a year-~55000 instant and croner aborts with
 *  "time search limit exceeded". Convert to seconds at the seam. */
function nowSecs(): number {
  return Math.floor(Date.now() / 1000);
}

export function useReminders() {
  const [state, setState] = useState<State>({
    reminders: [],
    loading: true,
    error: null,
  });

  const refresh = useCallback(async () => {
    setState((s) => ({ ...s, loading: true, error: null }));
    try {
      const reminders = await listReminders();
      setState({ reminders, loading: false, error: null });
    } catch (e) {
      setState((s) => ({
        ...s,
        loading: false,
        error: e instanceof Error ? e.message : String(e),
      }));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const create = useCallback(
    async (
      id: string,
      schedule: string,
      action: ReminderAction,
      opts?: { maxRuns?: number | null },
    ) => {
      await createReminder(id, schedule, action, { ...opts, ts: nowSecs() });
      await refresh();
    },
    [refresh],
  );

  const update = useCallback(
    async (id: string, patch: Parameters<typeof updateReminder>[1]) => {
      await updateReminder(id, { ...patch, ts: nowSecs() });
      await refresh();
    },
    [refresh],
  );

  const remove = useCallback(
    async (id: string) => {
      await deleteReminder(id, nowSecs());
      await refresh();
    },
    [refresh],
  );

  // Run-now — fire the reminder immediately, independent of its schedule, then refresh so the run
  // count / next-attempt reflect the firing. Returns the fire result so the caller can surface a
  // deny (the documented dev-login run-now limitation) without treating it as a page-level error.
  const fire = useCallback(
    async (id: string) => {
      const res = await fireReminder(id, nowSecs());
      await refresh();
      return res;
    },
    [refresh],
  );

  return { ...state, refresh, create, update, remove, fire };
}
