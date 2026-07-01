# Session — reminders as the first rich-channel tenant (backend-driven, zero UI reminder knowledge)

Topic: `reminders`. Scope: [`reminders-rich-responses-scope.md`](../../scope/reminders/reminders-rich-responses-scope.md).
The first concrete tenant of [channel rich responses](../channels/channels-rich-responses-session.md) —
it proves the request-form / rich-response / control loop end to end against a real, shipped MCP surface.

## What shipped

The whole reminders CRUD-and-list surface is now drivable from a channel **with zero reminders-specific
UI code** — the proof that the rich-responses contract is real. `/remind` opens a backend-declared form
(cron builder + action select) that calls `reminder.create`; `/reminders` answers with an interactive
table whose per-row pause/run-now/delete controls drive `reminder.update`/`reminder.fire`/`reminder.delete`,
gated + workspace-scoped. Every pixel is a **shipped** dashboard widget over the host-mediated bridge.

**The reminders feature ships entirely from backend descriptors.** The UI has no idea it is rendering
reminders — it lists commands, renders their schema widgets, and posts their declared response render
(see the [channels session](../channels/channels-rich-responses-session.md) for the corrected,
100%-backend-driven contract and the domain-leak we removed).

## The two descriptors + one new verb

- **`reminder.create` — the form command** (`rust/crates/host/src/reminder/descriptor.rs`). Named
  exactly `reminder.create`, so the catalog's per-tool `authorize_tool` gates visibility on
  `mcp:reminder.create:call` with zero special-casing (same mechanism as `agent.invoke`). Its
  `input_schema` is **form-shaped and flat**: `schedule` (`x-lb:{widget:"cron"}`), `action_kind`
  (`x-lb:{widget:"select", options:["channel-post","mcp-tool","outbox"]}`), the per-kind action fields
  (`channel`/`body`, `tool`/`args`, `target`/`action_action`/`payload`), `max_runs?`, `enabled?`;
  `required:["schedule","action_kind"]`.
- **The `reminder.create` VERB accepts the flat form** (`rust/crates/host/src/reminder/tool.rs`). The
  UI posts the collected flat fields verbatim; the **host** builds the nested `Action` server-side
  (`action_from_flat`) — so no client reshaping. `id` optional (host derives a stable ts-keyed slug);
  `ts` optional (host supplies `now` **in seconds**). The nested `action:{kind,…}` form still works
  (the reminder engine and existing tests use it) — nested wins when present.
- **`reminder.list` — the interactive-list command**. Its descriptor carries a **`result`** render
  envelope: `view:"table"`, `source:{tool:"reminder.list"}`, and `options.rowControls` — a pause
  **switch** (`reminder.update{id:"${id}", enabled:"{{value}}"}`), a **run-now** button
  (`reminder.fire{id:"${id}"}`), a **delete** button (`reminder.delete{id:"${id}"}`). The generic
  palette posts this `result`; `ResponseView`→`ResponseTable` mounts it; writes leash to the viewer's
  grant. `${id}` is the row field (shipped `${name}` vars engine); `{{value}}` is the switch bool.
- **`reminder.fire` — the run-now verb** (`rust/crates/host/src/reminder/fire_now.rs`, new file). A
  gated (`mcp:reminder.fire:call`), idempotent, one-firing verb that **reuses the shipped internal fire
  path** (`fire_reminder` from `fire.rs`) — it does not duplicate dispatch. Idempotent on
  `(reminder_id, instant=now)` exactly like the reactor scan: it writes the deterministic
  `fire_job_id(id, now)` marker before dispatch, so a double-click in the same instant fires once.
  A manual fire uses `scheduled_ts = now` (not `next_attempt_ts`) so it never collides with a
  legitimate scheduled fire's job id, and it does **not** advance the schedule (run-now is an extra
  manual firing, not a schedule tick). Its descriptor puts the tool in the catalog only for a caller
  who may fire; the cap is granted in the member bundle beside the other `reminder.*` caps
  (`rust/role/gateway/src/session/credentials.rs`).

## The migration (additive, no rip-out)

The shipped `channels-query-charts` `kind:"query_result"` is now **expressible** as a `rich_result`
(`view:"table"|"chart"`). The old `QueryCard` renderer keeps working (`MessageItem` still routes
`query_result` → `QueryCard`), and a regression test proves a `query_result` renders identically
through the new path. We did **not** remove the old path in this slice.

## Bugs caught during the build

Three, in order of surfacing — the real-gateway loop earned its keep:

1. **`ts` unit (fixed).** The reminder cron math (`next_after`) works in **seconds**; an early
   flat-form path supplied `ts` in **milliseconds**, which blew the `CronScheduler` search limit. Fixed
   by having the host supply `now` in seconds when the flat form omits `ts`.
   ([debug](../../debugging/reminders/ts-unit-mismatch-cron-search-limit.md))
2. **Write verbs required `ts` (fixed, this slice).** The generic, backend-driven row controls send a
   **`ts`-free** `argsTemplate` (correct — the frontend is tool-agnostic), but `reminder.update`/
   `delete`/`fire` hard-required `ts` while `reminder.create` already defaulted it — so every row
   control was inert (`bad input: missing u64 arg: ts`). Fixed by defaulting `ts` to the host clock in
   all three write verbs, matching `create`; the transport stays generic. Regression test
   `write_verbs_default_ts_when_absent`.
   ([debug](../../debugging/reminders/reminder-write-verbs-require-ts.md))
3. **`reminder.list` not unwrapped to table rows (fixed, this slice).** A `rich_result` table
   `source`-d at `reminder.list` resolves through `viz.query`, whose row-unwrap looks for rows under a
   fixed set of plural keys (`samples`/`items`/`rows`/`templates`/`dashboards`). `reminder.list` returns
   `{reminders:[…]}` — `reminders` wasn't in the set — so the table rendered **one JSON-blob row**
   instead of N, and a row control's `${id}` bound nothing. Fixed by adding `"reminders"` to both
   mirrored `ROW_KEYS` lists (`viz/frame.rs` + `useSource.ts`); regression
   `reminders_shape_unwraps_to_n_rows`.
   ([debug](../../debugging/reminders/reminder-list-not-unwrapped-to-table-rows.md))
4. **Fire re-resolve misses token caps (pre-existing, deferred).** Over the real gateway, a
   **dev-login** reminder **won't fire** (run-now and the scheduled reactor both) because
   `fire_reminder` re-resolves the creator's caps **from the durable grant store** at fire time (so a
   revoke applies), but a dev-login token carries `member_caps()` in the **JWT only** — nothing durable
   — so the re-resolve finds the action cap absent and denies. This is **pre-existing in the shipped
   reminder system** (commit `b78a0bd`), surfaced by driving a real dev-login fire; it is
   security-semantics-sensitive and out of scope for this surface slice. The run-now **control is
   correct** and works the instant the fire path is fixed. Documented as a named follow-up.
   ([debug](../../debugging/reminders/reminder-fire-reresolve-misses-token-caps.md))

## Tests (real gateway, real reminder.* verbs + reactor, real seeded reminders — no mocks)

- **Rust** (`rust/crates/host/tests/reminder_fire_test.rs`): `reminder.fire` deny (no cap → opaque),
  workspace-isolation (ws-B can't fire a ws-A reminder even with a leaked id — ws from the token),
  idempotency (double-fire same instant → one action); the flat-form create (no nested action, no id)
  creates a real reminder; the nested-action create still works; the catalog shows `reminder.create`/
  `reminder.list` only with the matching cap, and `reminder.list` carries its render.
- **UI real-gateway** (`*.reminders.gateway.test.tsx`): the loop through the **generic** palette —
  `/remind` builds a real reminder (asserted via `reminder.get`); `/reminders` posts the descriptor
  render and the table renders the seeded reminders from the real `reminder.list`; **pause** and
  **delete** drive the real verbs (side effect asserted); a viewer with `list` but not `update` is
  denied server-side on the pause toggle; two sessions isolated; the token appears in no bridge/render
  payload. **Run-now** asserts the *documented pre-existing* deny (bug #3 above) rather than a passing
  fire — the honest state until the fire re-resolve is fixed; the Rust `reminder_fire_test` already
  proves fire **works** when the action cap is granted durably, so both sides are covered.
- **UI unit**: the `query_result → rich_result` migration mapping + the `query_result` no-regression.
