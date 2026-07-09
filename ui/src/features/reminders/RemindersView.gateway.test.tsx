// The reminders page, driven against a REAL in-process gateway (reminders scope; CLAUDE §9 /
// testing §0 — no fake backend). Each test signs in to a UNIQUE workspace with an explicit real cap
// set and drives the real RemindersView + useReminders hook + reminders.api client + HTTP transport
// against the real `reminder.*` host verbs + the real `reminder:{id}` store records. It proves the
// full UI-side CRUD surface round-trips the real path: author (create) → list → pause/resume
// (update) → delete (tombstone), each through the host-mediated `POST /mcp/call` bridge.
//
// The per-verb capability-deny + workspace-isolation are proven SERVER-SIDE in the Rust integration
// tests (`reminders_mcp_test.rs`: each_verb_is_denied_without_its_grant,
// workspace_isolation_list_and_get_never_cross_the_wall) and the reactor test
// (`reminders_reactor_test.rs`). This file proves the UI drives the real verbs, not a fake.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { RemindersView } from "./RemindersView";
import { getReminder, listReminders } from "@/lib/reminders/reminders.api";
import { useRealGateway, signInWithCaps } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `reminders-ui-${n++}`;

// The reminders cap set: the five `reminder.*` MCP caps the CRUD verbs re-check below the bridge.
const REMINDER_CAPS = [
  "mcp:reminder.create:call",
  "mcp:reminder.update:call",
  "mcp:reminder.delete:call",
  "mcp:reminder.get:call",
  "mcp:reminder.list:call",
];

beforeAll(() => {
  useRealGateway();
});

async function signIn(ws: string) {
  await signInWithCaps("user:ada", ws, REMINDER_CAPS);
}

/** Author one channel-post reminder through the real create dialog. Opens "New reminder", leaves the
 *  default schedule (`0 8 * * 0,1`) — the cron widget round-trips it; we only fill id + the action
 *  body — then submits. The submit closes the dialog and the real `reminder.list` refresh lists it. */
async function authorReminder(
  user: ReturnType<typeof userEvent.setup>,
  id: string,
  channel: string,
  body: string,
) {
  await user.click(screen.getByLabelText("new reminder"));
  await user.type(await screen.findByLabelText("reminder id"), id);
  // The default action kind is channel-post; fill its two fields.
  await user.type(screen.getByPlaceholderText("channel (e.g. team)"), channel);
  await user.type(screen.getByPlaceholderText("message body"), body);
  await user.click(screen.getByLabelText("create reminder"));
}

describe("RemindersView (real gateway)", () => {
  it("creates a reminder via the real path and lists it", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signIn(ws);

    render(<RemindersView ws={ws} />);
    await authorReminder(user, "standup-ping", "team", "standup time");

    // It appears in the list (driven by the real `reminder.list`). The schedule round-trips through
    // the react-js-cron builder, which normalizes equivalent forms (`0,1` → `0-1`) losslessly.
    const row = await screen.findByLabelText("reminder standup-ping");
    expect(within(row).getByText(/^0 8 \* \* 0[,-]1$/)).toBeInTheDocument();
    expect(within(row).getByText(/post → #team/)).toBeInTheDocument();

    // Persisted: re-fetch through the real `reminder.get` path — the round-trip is faithful, and the
    // host computed a future `nextAttemptTs` from the cron string on the injected clock.
    const fetched = await getReminder("standup-ping");
    expect(fetched).not.toBeNull();
    expect(fetched!.schedule).toMatch(/^0 8 \* \* 0[,-]1$/);
    expect(fetched!.action).toMatchObject({ kind: "channel-post", channel: "team", body: "standup time" });
    expect(fetched!.enabled).toBe(true);
    expect(fetched!.nextAttemptTs).toBeGreaterThan(0);
  });

  it("pauses and resumes a reminder via the real update verb", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signIn(ws);

    render(<RemindersView ws={ws} />);
    await authorReminder(user, "nightly", "ops", "rollup");

    const row = await screen.findByLabelText("reminder nightly");
    expect(within(row).getByLabelText("reminder nightly status").textContent).toContain("enabled");

    // Pause (real `reminder.update` with enabled=false), then assert the store reflects it.
    await user.click(within(row).getByLabelText("toggle reminder nightly"));
    await screen.findByText(/paused/);
    expect((await getReminder("nightly"))!.enabled).toBe(false);

    // Resume — back to enabled, proven through the real get.
    await user.click(within(await screen.findByLabelText("reminder nightly")).getByLabelText("toggle reminder nightly"));
    await screen.findByText(/enabled/);
    expect((await getReminder("nightly"))!.enabled).toBe(true);
  });

  it("deletes a reminder (tombstone) via the real delete verb", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signIn(ws);

    render(<RemindersView ws={ws} />);
    await authorReminder(user, "to-delete", "team", "bye");

    const row = await screen.findByLabelText("reminder to-delete");
    await user.click(within(row).getByLabelText("delete reminder to-delete"));
    // Deletes route through ConfirmDestructive — confirm the (no-escalation) gate. Its confirm
    // button carries aria-label="confirm action" (the accessible name wins over the visible label).
    await user.click(await screen.findByLabelText("confirm action"));

    // Gone from the list (the real `reminder.list` no longer returns the tombstoned row)…
    await waitForGone("reminder to-delete");
    // …and a direct `reminder.get` returns null (the tombstone is honored end to end).
    expect(await getReminder("to-delete")).toBeNull();
    // The list verb agrees.
    expect((await listReminders()).find((r) => r.id === "to-delete")).toBeUndefined();
  });
});

/** Poll the rendered list until a row disappears (the refresh after a delete is async). */
async function waitForGone(label: string) {
  await screen.findByLabelText("reminder list");
  for (let i = 0; i < 50; i++) {
    if (!screen.queryByLabelText(label)) return;
    await new Promise((r) => setTimeout(r, 20));
  }
  expect(screen.queryByLabelText(label)).toBeNull();
}
