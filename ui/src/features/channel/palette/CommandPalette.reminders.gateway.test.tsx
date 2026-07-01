// The reminders tenant driven through the GENERIC command palette, against a REAL spawned gateway (no
// fake — CLAUDE §9). The palette has ZERO reminder knowledge: it lists commands from `tools.catalog`,
// renders each command's JSON-Schema arg widgets, and on submit either POSTS the command's declared
// `descriptor.result` render envelope (a `tool.result` present) or makes a plain `{tool,args}` bridge
// call. This file drives that real generic path and asserts the real side effects through the real
// reminder.* verbs + reactor — nothing is mocked.
//
// SCOPE OF THIS FILE (honest — ONE shipped limitation is asserted as a limitation, not faked green):
//   Run-now (`reminder.fire`) via a dev-login is DENIED at the host. This is the PRE-EXISTING
//   fire-re-resolve bug (the firing re-resolves the stored principal's caps from the durable grant
//   store, which a dev-login token does not populate) — documented in
//   docs/debugging/reminders/reminder-fire-reresolve-misses-token-caps.md and OUT OF SCOPE for this
//   slice. The Rust `reminder_fire_test.rs` proves fire WORKS when the action cap is granted durably;
//   here we assert the documented deny, NOT a passing run-now.
//
// The interactive-list RENDER + controls now work END TO END (the row-unwrap gap — `reminder.list`
// returning `{reminders:[…]}` — was fixed by adding `reminders` to the shipped ROW_KEYS in both mirrors,
// `viz/frame.rs` + `useSource.ts`, per docs/debugging). So this file mounts the posted rich_result
// through MessageItem→ResponseView→ResponseTable and asserts N per-reminder rows AND drives the actual
// per-row controls VIA DOM interaction (the pause switch / the delete button), asserting the real side
// effect through `reminder.get`.
//
// What round-trips end to end here (real gateway, real verbs): create (flat form), the list command's
// posted rich_result envelope, the N-row interactive-list render, DOM-driven pause (reminder.update) and
// delete (reminder.delete), the capability-deny headline, workspace isolation, and token-never-crosses.

import { describe, expect, it, beforeAll, vi } from "vitest";
import { render, screen, within, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { CommandPalette } from "./CommandPalette";
import { MessageItem } from "../MessageItem";
import type { Item } from "@/lib/channel/channel.types";
import { encodeRichResult, parsePayload, type RichResultPayload } from "@/lib/channel/payload.types";
import type { ToolDescriptor } from "@/lib/channel/palette.types";
import { interpolateArgs, type VarScope } from "@/lib/vars";
import { invoke } from "@/lib/ipc/invoke";
import { sessionToken } from "@/lib/session/session.store";
import { useRealGateway, signInReal, signInWithCaps } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `reminders-${n++}`;

/** Seed one real reminder through the REAL `reminder.create` verb (the flat descriptor form — no ts/id,
 *  the host derives both), returning the derived id. This is the same call the generic palette makes. */
async function seedReminder(channel: string, body: string): Promise<string> {
  const r = await invoke<{ id: string }>("mcp_call", {
    tool: "reminder.create",
    args: { schedule: "0 8 * * 1", action_kind: "channel-post", channel, body },
  });
  return r.id;
}

/** Read one reminder back through the real `reminder.get` verb. */
async function getReminder(id: string) {
  const r = await invoke<{ reminder: Record<string, unknown> | null }>("mcp_call", {
    tool: "reminder.get",
    args: { id },
  });
  return r.reminder;
}

/** List every reminder in the session workspace through the real `reminder.list` verb. */
async function listReminders() {
  const r = await invoke<{ reminders: Array<Record<string, unknown>> }>("mcp_call", {
    tool: "reminder.list",
    args: {},
  });
  return r.reminders;
}

/** Read the real catalog the palette renders from (`tools.catalog`) — the caller's authorized tools. */
async function catalog(): Promise<ToolDescriptor[]> {
  const c = await invoke<{ tools: ToolDescriptor[] }>("mcp_call", { tool: "tools.catalog", args: {} });
  return c.tools;
}

const noop = () => {};

/** The reminder.list descriptor's declared render — the exact envelope the generic palette posts (and
 *  the `list command posts …` test asserts it is what the descriptor carries). Used to MOUNT the
 *  interactive list here, so we render the same thing a real posted Item carries. */
const LIST_RENDER_BODY = encodeRichResult({
  v: 2,
  view: "table",
  source: { tool: "reminder.list", args: {} },
  options: {
    rowControls: [
      { kind: "switch", label: "enabled", action: { tool: "reminder.update", argsTemplate: { id: "${id}", enabled: "{{value}}" } } },
      { kind: "button", buttonLabel: "Run now", action: { tool: "reminder.fire", argsTemplate: { id: "${id}" } } },
      { kind: "button", buttonLabel: "Delete", action: { tool: "reminder.delete", argsTemplate: { id: "${id}" } } },
    ],
  },
  // The per-field PRESENTATION the reminder.list descriptor declares (widget-kit scope) — mirrors
  // rust/crates/host/src/reminder/descriptor.rs `list_render()` fieldConfig ONE-TO-ONE. The palette posts
  // this verbatim off the catalog; here we mount the same envelope so the table resolves the same headers.
  fieldConfig: {
    defaults: {},
    overrides: [
      { matcher: { id: "byName", options: "maxRuns" }, properties: [ { id: "displayName", value: "Max Runs" }, { id: "description", value: "Stop after N fires (blank = forever)" } ] },
      { matcher: { id: "byName", options: "nextAttemptTs" }, properties: [ { id: "displayName", value: "Next fire" } ] },
      { matcher: { id: "byName", options: "action" }, properties: [ { id: "displayName", value: "Action" }, { id: "description", value: "What fires" } ] },
      { matcher: { id: "byName", options: "principalSub" }, properties: [ { id: "hide", value: true } ] },
      { matcher: { id: "byName", options: "ts" }, properties: [ { id: "hide", value: true } ] },
    ],
  },
  tools: ["reminder.list", "reminder.update", "reminder.fire", "reminder.delete"],
});

/** Mount the reminder.list rich_result through the REAL channel render path (MessageItem →
 *  ResponseView → ResponseTable), wait for the source (`reminder.list` via viz.query) to load, and return
 *  the mounted `<table>`. This is the interactive list a channel member sees. */
async function mountList(ws: string): Promise<HTMLElement> {
  const item: Item = { id: "rich-list", channel: "general", author: "system:reminders", body: LIST_RENDER_BODY, ts: 1 };
  render(<MessageItem item={item} author="user:me" ws={ws} onEdit={noop} onDelete={noop} />);
  return waitFor(() => {
    const t = document.querySelector('[aria-label="response table"]');
    if (!t || t.querySelectorAll("tbody tr").length === 0) throw new Error("table not loaded");
    return t as HTMLElement;
  }, { timeout: 8000 });
}

/** Mount the reminder.list interactive list from the REAL descriptor's declared render — pulled off the
 *  live `tools.catalog` (NOT the hand-mirrored constant), so this proves the Rust `list_render()` (incl.
 *  its widget-kit `fieldConfig`) reaches the rendered table end to end: descriptor → catalog → posted
 *  rich_result → ResponseView → ResponseTable. The palette posts `tool.result` verbatim; we do the same. */
async function mountListFromCatalog(ws: string): Promise<HTMLElement> {
  const tools = await catalog();
  const list = tools.find((t) => t.name === "reminder.list");
  if (!list?.result) throw new Error("reminder.list descriptor carries no result render");
  const body = encodeRichResult(list.result as Parameters<typeof encodeRichResult>[0]);
  const item: Item = { id: "rich-list-cat", channel: "general", author: "system:reminders", body, ts: 1 };
  render(<MessageItem item={item} author="user:me" ws={ws} onEdit={noop} onDelete={noop} />);
  return waitFor(() => {
    const t = document.querySelector('[aria-label="response table"]');
    if (!t || t.querySelectorAll("tbody tr").length === 0) throw new Error("table not loaded");
    return t as HTMLElement;
  }, { timeout: 8000 });
}

/** The index of the mounted row whose text contains `id` (the row for a specific reminder). */
function rowIndexOf(table: HTMLElement, id: string): number {
  const rows = Array.from(table.querySelectorAll("tbody tr"));
  return rows.findIndex((r) => r.textContent?.includes(id));
}

beforeAll(() => useRealGateway());

describe("Reminders through the generic palette (real gateway)", () => {
  it("create round-trip: the flat descriptor form creates a real reminder (host derives id + now)", async () => {
    // The generic palette posts the reminder.create descriptor's FLAT form VERBATIM through the bridge
    // (`onCallTool` → mcp_call), with NO ts and NO id — the host assembles the Action, supplies `now`,
    // and derives a stable id. We drive that exact bridge call and assert the REAL stored reminder.
    const ws = nextWs();
    await signInReal("user:me", ws);

    // The collected flat form the palette's generic submit passes to onCallTool for reminder.create
    // (schedule cron + action_kind:"channel-post" + the per-kind channel/body — the descriptor's shape).
    await invoke("mcp_call", {
      tool: "reminder.create",
      args: { schedule: "0 8 * * 1", action_kind: "channel-post", channel: "standup", body: "time to sync" },
    });

    // A real reminder now exists (read it back through reminder.get / list — the UI sent no ts/id).
    const reminders = await listReminders();
    expect(reminders.length).toBe(1);
    const created = reminders[0] as {
      id: string;
      schedule: string;
      action: { kind: string; channel: string; body: string };
    };
    expect(String(created.id)).toMatch(/^reminder-post-standup-/); // host-derived, ts-keyed, no uuid
    expect(created.schedule).toBe("0 8 * * 1");
    expect(created.action).toMatchObject({ kind: "channel-post", channel: "standup", body: "time to sync" });

    // And through reminder.get by the derived id.
    const got = (await getReminder(created.id)) as { action: { kind: string } } | null;
    expect(got?.action.kind).toBe("channel-post");
  });

  it("create form e2e: driving the /remind palette form creates a real channel-post reminder", async () => {
    // The FULL request-side round-trip through the REAL generic palette form (no direct invoke): accept
    // the reminder.create command from the real catalog, fill the cron `schedule` (its default is seeded),
    // let the `action_kind` select default to "channel-post", fill the CONDITIONALLY-required `channel`
    // (surfaced only because it declared `x-lb.showIf`+`requiredWhenShown`), and submit. The palette makes
    // the plain bridge call VERBATIM; we assert the real stored reminder through reminder.list/get. This is
    // the surface Bug B left unreachable — the action fields could not be filled from the form before.
    const ws = nextWs();
    await signInReal("user:me", ws);

    const user = userEvent.setup();
    const calls: Array<{ tool: string; args: Record<string, unknown> }> = [];
    render(
      <CommandPalette
        channel="general"
        onPostQuery={noop}
        onSendAgent={noop}
        onCallTool={async (tool, args) => {
          calls.push({ tool, args });
          await invoke("mcp_call", { tool, args }); // the real host-mediated bridge
        }}
        onPostRich={noop}
        onSendChat={noop}
      />,
    );

    // Accept /reminder.create from the REAL catalog.
    await user.type(screen.getByLabelText("message"), "/reminder");
    const menu = await screen.findByRole("listbox", { name: "commands" });
    await user.click(within(menu).getByText(/Schedule a reminder/));

    // The cron `schedule` seeds its default; the `action_kind` select preselects "channel-post" → the
    // conditionally-required `channel` field surfaces. Fill it and submit.
    const channel = await screen.findByLabelText("channel");

    // FORM presentation (widget-kit): the arg rail renders the field's resolved label + description from
    // the SAME resolver the table headers use — the descriptor declared `channel → {label:"Channel",
    // description:"The channel to post into"}`, so the rail shows them (not the raw `channel`).
    const fieldRow = await screen.findByLabelText("field channel");
    expect(fieldRow.textContent).toContain("Channel");
    expect(fieldRow.textContent).toContain("The channel to post into");

    await user.type(channel, "standup");
    await waitFor(() => expect(screen.getByLabelText("send")).toBeEnabled());
    await user.click(screen.getByLabelText("send"));

    // The palette made ONE plain bridge call with the collected form (cron default + kind + channel) —
    // NO stale per-kind fields for the other action kinds (only shown fields collected).
    await waitFor(() => expect(calls.length).toBe(1));
    expect(calls[0].tool).toBe("reminder.create");
    expect(calls[0].args).toMatchObject({ schedule: "0 9 * * *", action_kind: "channel-post", channel: "standup" });
    expect(calls[0].args).not.toHaveProperty("target"); // an outbox field never leaked
    expect(calls[0].args).not.toHaveProperty("tool"); // an mcp-tool field never leaked

    // A REAL reminder now exists, with the channel-post action assembled host-side from the flat form.
    const reminders = await listReminders();
    expect(reminders.length).toBe(1);
    const created = reminders[0] as { id: string; schedule: string; action: { kind: string; channel: string } };
    expect(created.schedule).toBe("0 9 * * *");
    expect(created.action).toMatchObject({ kind: "channel-post", channel: "standup" });
    expect((await getReminder(created.id))?.action).toMatchObject({ kind: "channel-post", channel: "standup" });
  });

  it("list command posts the descriptor.result render envelope VERBATIM (tool-agnostic)", async () => {
    // Drive the REAL generic palette: `/reminder.list` → accept the command from the REAL catalog →
    // submit. Because reminder.list's descriptor DECLARES a `result`, the palette POSTS that render (it
    // makes NO reminder-specific branch). We assert the posted body IS the descriptor.result: a v2
    // table over the reminder.list source with the three row controls bound to update/fire/delete.
    const ws = nextWs();
    await signInReal("user:me", ws);
    await seedReminder("team", "one");

    const user = userEvent.setup();
    const posted: string[] = [];
    render(
      <CommandPalette
        channel="general"
        onPostQuery={noop}
        onSendAgent={noop}
        onCallTool={vi.fn()}
        onPostRich={(b) => { posted.push(b); }}
        onSendChat={noop}
      />,
    );

    await user.type(screen.getByLabelText("message"), "/reminder");
    const menu = await screen.findByRole("listbox", { name: "commands" });
    await user.click(within(menu).getByText(/List reminders/));
    // The list command's only args are OPTIONAL (status/limit), so no arg box demands input — the command
    // is runnable the instant it is picked (the optional-arg fix). Press send: the palette posts the
    // declared render. The palette itself carries no reminder knowledge — it just posts `tool.result`.
    expect(screen.queryByLabelText("status")).toBeNull();
    await user.click(screen.getByLabelText("send"));
    await waitFor(() => expect(posted.length).toBe(1));

    const payload = parsePayload(posted[0]) as RichResultPayload;
    expect(payload.kind).toBe("rich_result");
    expect(payload.v).toBe(2);
    expect(payload.view).toBe("table");
    expect(payload.source).toMatchObject({ tool: "reminder.list" });
    // The three declared row controls, bound to the reminder write verbs (${id} row field, {{value}} bool).
    const controls = (payload.options?.rowControls ?? []) as Array<{
      kind: string;
      action: { tool: string; argsTemplate: Record<string, unknown> };
      buttonLabel?: string;
      label?: string;
    }>;
    expect(controls.length).toBe(3);
    expect(controls[0]).toMatchObject({
      kind: "switch",
      action: { tool: "reminder.update", argsTemplate: { id: "${id}", enabled: "{{value}}" } },
    });
    expect(controls[1].action.tool).toBe("reminder.fire");
    expect(controls[2].action.tool).toBe("reminder.delete");
    expect(payload.tools).toEqual([
      "reminder.list",
      "reminder.update",
      "reminder.fire",
      "reminder.delete",
    ]);
  });

  it("list render: the mounted rich_result shows N per-reminder rows from the real reminder.list source", async () => {
    // Mount the posted reminder.list rich_result through the REAL channel render path
    // (MessageItem→ResponseView→ResponseTable). The table re-runs its `source` (reminder.list via
    // viz.query) and — with the row-unwrap fix — renders ONE ROW PER REMINDER (not a single JSON-blob
    // row). We assert the row count == N and that each seeded reminder's id + schedule appears.
    const ws = nextWs();
    await signInReal("user:me", ws);
    const ids = [
      await seedReminder("team", "one"),
      await seedReminder("ops", "two"),
      await seedReminder("sre", "three"),
    ];

    const table = await mountList(ws);
    const rows = table.querySelectorAll("tbody tr");
    expect(rows.length).toBe(3); // N per-reminder rows, NOT one {reminders:[…]} blob row

    // Each seeded reminder is a distinct row carrying its real data (id + the seeded schedule column).
    for (const id of ids) {
      expect(rowIndexOf(table, id)).toBeGreaterThanOrEqual(0);
    }
    // The columns resolve through the ONE presentation resolver (widget-kit): raw keys humanize
    // ("Id"/"Schedule"), never a lone `reminders` column, and the actual schedule shows in the rows.
    const headers = Array.from(table.querySelectorAll("thead th")).map((h) => h.textContent);
    expect(headers).toContain("Id");
    expect(headers).toContain("Schedule");
    expect(headers).not.toContain("reminders"); // the array was unwrapped, not shown as one cell
    expect(table.textContent).toContain("0 8 * * 1"); // the real seeded schedule shows in the rows
  });

  it("presentation regression (the motivating fix): /reminders table shows 'Max Runs', hides principalSub/ts, no raw action blob", async () => {
    // The HEADLINE widget-kit fix over the REAL gateway. reminder.list's descriptor declares a
    // `fieldConfig` (Max Runs / Next fire / Action, principalSub+ts hidden). The rich_result carries it,
    // buildCell copies it onto the cell, and the shared table column-model resolves every header through
    // the ONE presentation resolver. So the table reads author labels — NOT raw record keys — and drops
    // the hidden columns. `action` renders as a labeled cell (readable text), never a thrown JSON blob.
    const ws = nextWs();
    await signInReal("user:me", ws);
    await seedReminder("standup", "sync");

    // Mount from the REAL descriptor's declared render (off the live catalog), so this proves the Rust
    // list_render() fieldConfig reaches the table end to end — not a test-authored envelope.
    const table = await mountListFromCatalog(ws);
    const headers = Array.from(table.querySelectorAll("thead th")).map((h) => h.textContent);

    // Author label wins over the humanized raw key: "Max Runs", never "maxRuns".
    expect(headers).toContain("Max Runs");
    expect(headers).not.toContain("maxRuns");
    // The nextAttemptTs override reads "Next fire"; action reads "Action".
    expect(headers).toContain("Next fire");
    expect(headers).toContain("Action");
    // Hidden columns are DROPPED from the surface (presentation, not security — see the deny test).
    expect(headers).not.toContain("principalSub");
    expect(headers).not.toContain("Principal Sub");
    expect(headers).not.toContain("ts");
    // The `action` cell is readable text (the seeded channel-post shows its data), not a header-less blob
    // dumped as one giant column — a real value appears in the rows under the Action column.
    expect(table.textContent).toContain("standup");

    // HIDE IS NOT SECURITY: the hidden field still crossed the bridge under the viewer's grant — the raw
    // reminder.list row STILL contains principalSub (hiding only drops the rendered column).
    const rows = await listReminders();
    expect(rows[0]).toHaveProperty("principalSub"); // data crossed; only the column is hidden
  });

  it("pause control DOM e2e: clicking a row's switch drives reminder.update → enabled:false", async () => {
    // Drive the ACTUAL mounted per-row control via DOM. The shipped SwitchControl has no per-row state
    // source, so it starts OFF: click 1 sends enabled:true (a no-op on an already-enabled reminder),
    // click 2 sends enabled:false — the real pause. `${id}` now binds the correct row id (row-unwrap fix),
    // so the write targets THIS reminder. We assert the real side effect through reminder.get.
    const ws = nextWs();
    await signInReal("user:me", ws);
    const id = await seedReminder("team", "pausable");
    const other = await seedReminder("ops", "keep-me");

    const table = await mountList(ws);
    const idx = rowIndexOf(table, id);
    expect(idx).toBeGreaterThanOrEqual(0);
    const toggles = table.querySelectorAll('[aria-label="toggle"]');
    const toggle = toggles[idx] as HTMLElement;

    const user = userEvent.setup();
    await user.click(toggle); // → reminder.update { id, enabled: true }
    await user.click(toggle); // → reminder.update { id, enabled: false }
    await waitFor(async () => {
      const got = (await getReminder(id)) as { enabled: boolean } | null;
      expect(got?.enabled).toBe(false); // the pause really took effect on THIS reminder
    });
    // The OTHER reminder is untouched — `${id}` bound the clicked row only (not a shared/blob scope).
    expect(((await getReminder(other)) as { enabled: boolean }).enabled).toBe(true);
  });

  it("delete control DOM e2e: clicking a row's Delete button drives reminder.delete → the row drops", async () => {
    // Drive the mounted Delete ButtonControl via DOM. `${id}` binds the clicked row's real id, so the
    // real reminder.delete removes THAT reminder; we assert reminder.get returns none and a re-mounted
    // list no longer shows it.
    const ws = nextWs();
    await signInReal("user:me", ws);
    const id = await seedReminder("team", "deletable");
    const survivor = await seedReminder("ops", "survivor");

    const table = await mountList(ws);
    const rows = Array.from(table.querySelectorAll("tbody tr"));
    const row = rows[rowIndexOf(table, id)];
    // The Delete control's shipped chrome is `aria-label="button reminder.delete"` with an inner "fire"
    // button; scope the query to the reminder's own row so we click ITS delete.
    const delBtn = row.querySelector('[aria-label="button reminder.delete"] [aria-label="fire"]') as HTMLElement;

    const user = userEvent.setup();
    await user.click(delBtn); // → reminder.delete { id }
    await waitFor(async () => {
      expect(await getReminder(id)).toBeNull(); // reminder.get returns none
    });

    // The survivor is still present, and a freshly-mounted list shows exactly it (the deleted one dropped).
    expect(await getReminder(survivor)).not.toBeNull();
    const remaining = await listReminders();
    expect(remaining.map((r) => r.id)).toEqual([survivor]);
  });

  it("run-now (reminder.fire) is DENIED for a dev-login — the documented pre-existing re-resolve bug", async () => {
    // HONEST assertion of a KNOWN LIMITATION, not a passing run-now. reminder.fire re-resolves the
    // reminder's stored principal caps from the DURABLE GRANT STORE (not the token), and a dev-login
    // token does not populate the store with the action's cap (`bus:chan/team:pub`), so the firing's
    // action re-check denies. This is PRE-EXISTING (shipped reminder system) and out of scope for this
    // slice — see docs/debugging/reminders/reminder-fire-reresolve-misses-token-caps.md. The Rust
    // `reminder_fire_test.rs` proves fire WORKS when the cap is granted durably, so both sides are covered.
    const ws = nextWs();
    await signInReal("user:me", ws);
    const id = await seedReminder("team", "fireable");

    // The run-now button template ({ id: "${id}" }) — the real bridge call the control makes.
    await expect(invoke("mcp_call", { tool: "reminder.fire", args: { id } })).rejects.toThrow(/denied/i);

    // No side effect: the reminder never fired (the deny is before dispatch).
    const inbox = await invoke<{ items: unknown[] }>("mcp_call", { tool: "inbox.list", args: { channel: "team" } });
    expect(inbox.items).toHaveLength(0);
  });

  it("deny headline: list-only caps render the list command but DENY reminder.update; no create command", async () => {
    // A viewer granted the catalog + reminder.list ONLY (not update/create). The list command is offered
    // (so the /reminders table would render), but the pause control's write (reminder.update) is denied
    // SERVER-SIDE and opaquely — and reminder.create is ABSENT from the catalog (no existence leak).
    const ws = nextWs();
    await signInWithCaps("user:viewer", ws, [
      "mcp:tools.catalog:call",
      "mcp:reminder.list:call",
      "bus:chan/general:pub",
      "bus:chan/general:sub",
    ]);

    const names = (await catalog()).map((t) => t.name);
    expect(names).toContain("reminder.list"); // list allowed → the command is offered
    expect(names).not.toContain("reminder.create"); // no create cap → absent, not greyed (no leak)
    expect(names).not.toContain("reminder.update"); // and no update cap → not offered either

    // Flipping the pause switch would call reminder.update — denied opaquely at the host regardless of
    // what reached the bridge (the render.tools ∩ grant intersection + the verb's own gate agree).
    await expect(
      invoke("mcp_call", { tool: "reminder.update", args: { id: "anything", enabled: false } }),
    ).rejects.toThrow(/denied/i);

    // HIDE IS PRESENTATION, NOT SECURITY: the reminder.list render HIDES principalSub/ts, but that changes
    // NOTHING about the deny — a source/action the viewer lacks is denied server-side whether or not a
    // field is hidden. The deny above is opaque and unchanged by the presentation `hide` on the envelope.
    await expect(
      invoke("mcp_call", { tool: "reminder.create", args: { schedule: "0 8 * * 1", action_kind: "channel-post", channel: "x", body: "y" } }),
    ).rejects.toThrow(/denied/i); // no create cap → denied, exactly as without any `hide` involved
  });

  it("workspace isolation: ws-B sees only its own reminders and cannot touch ws-A's", async () => {
    // ws-A creates reminder "x". A ws-B session sees an EMPTY /reminders (never ws-A's), and a control
    // whose args name ws-A's id is refused at the host — the workspace is the token's, never the item body.
    const wsA = nextWs();
    await signInReal("user:ada", wsA);
    const xId = await seedReminder("team", "secret-a");

    const wsB = nextWs();
    await signInReal("user:bob", wsB);
    expect(await listReminders()).toHaveLength(0); // ws-B sees none of ws-A's reminders

    // ws-B firing the leaked id → NotFound ("no such tool" opaque); update → the same wall.
    await expect(invoke("mcp_call", { tool: "reminder.update", args: { id: xId, enabled: false } })).rejects.toThrow();
    // A delete of an absent id in ws-B is idempotent, but it can never reach ws-A's namespace.
    await invoke("mcp_call", { tool: "reminder.delete", args: { id: xId } }).catch(() => {});

    // ws-A's reminder is untouched (the wall held).
    await signInReal("user:ada", wsA);
    const survived = await getReminder(xId);
    expect(survived).not.toBeNull();
    expect((survived as { action: { body: string } }).action.body).toBe("secret-a");
  });

  it("the session token never crosses into a bridge arg or a posted rich_result body", async () => {
    // The shell holds the token; the palette/controls carry only {tool,args}. Drive the list command
    // (posts a rich_result) + a control write, and assert the token string appears in NEITHER the posted
    // body NOR the bridged args.
    const ws = nextWs();
    await signInReal("user:me", ws);
    const id = await seedReminder("team", "tok");
    const token = sessionToken();
    expect(token.length).toBeGreaterThan(20);

    const user = userEvent.setup();
    const posted: string[] = [];
    const calledArgs: string[] = [];
    render(
      <CommandPalette
        channel="general"
        onPostQuery={noop}
        onSendAgent={noop}
        onCallTool={(tool, args) => {
          calledArgs.push(JSON.stringify({ tool, args }));
        }}
        onPostRich={(b) => { posted.push(b); }}
        onSendChat={noop}
      />,
    );
    await user.type(screen.getByLabelText("message"), "/reminder");
    const menu = await screen.findByRole("listbox", { name: "commands" });
    await user.click(within(menu).getByText(/List reminders/));
    await user.click(screen.getByLabelText("send")); // only-optional args → runnable immediately
    await waitFor(() => expect(posted.length).toBe(1));

    // A real control write's args (what the bridge forwards) — interpolated, token-free.
    const row = (await getReminder(id)) as Record<string, unknown>;
    const scope: VarScope = { values: row as VarScope["values"], builtins: {} };
    const updateArgs = JSON.stringify(interpolateArgs({ id: "${id}", enabled: "{{value}}" }, scope, false));

    expect(posted[0]).not.toContain(token);
    expect(updateArgs).not.toContain(token);
    for (const a of calledArgs) expect(a).not.toContain(token);
  });
});
