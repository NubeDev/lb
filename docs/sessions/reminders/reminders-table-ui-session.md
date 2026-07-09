# Reminders — table UI, full CRUD, stats (session)

Date: 2026-07-08. Scope: [`scope/reminders/reminders-scope.md`](../../scope/reminders/reminders-scope.md)
(SHIPPED backend). Public: [`public/reminders/reminders.md`](../../public/reminders/reminders.md).

## Ask

"Add a nice UI for reminders — a table + full CRUD + some stats/past results; add it to the sidebar;
make sure the settings sidebar hide/show covers it."

## What was already there (no change needed)

The reminders feature was already scaffolded and wired end to end:

- **Sidebar entry** — `reminders` is a `CoreSurface` in `ui/src/features/shell/surfaceDefs.ts`
  (`CalendarClock`, label "Reminders") and sits in the **Automation** group in
  `ui/src/features/shell/NavRail.tsx` (`items: ["rules", "flows", "reminders"]`). Route registered in
  `ui/src/features/routing/createAppRouter.tsx`.
- **Settings hide/show** — `ui/src/features/settings/SidebarTab.tsx` renders one Switch per
  `SURFACE_GROUPS`/`SURFACE_DEF` entry, so Reminders appears in the hide/show list **automatically**;
  no separate registration. Confirmed both were correct before touching anything — the ask's
  sidebar/settings parts were already satisfied.

So the real work was **upgrading the basic author-rail + `<ul>` list** into a proper table with full
CRUD and a stats strip.

### BUT the sidebar entry was silently missing — a cap-gate bug (fixed)

Reported symptom: "I can't see Reminders in the sidebar." Root cause: the rail entry is gated by
`hasCap(caps, CAP.reminderList)` = an **exact** `caps.includes("mcp:reminder.list:call")`
(`ui/src/features/routing/allowed.ts` + `lib/session/admin-caps.ts`) — the frontend `hasCap` does
**not** expand the `mcp:*.list:call` wildcard the member token carries. The built-in member role
(`rust/crates/host/src/authz/builtin_roles.rs`, the base of dev-login) spelled out
`rules.list`/`flows.list`/`datasource.list`/`insight.list` explicitly but had only
`mcp:reminder.fire:call` for reminders — **not** `mcp:reminder.list:call`. So the rail filtered
Reminders out even though every verb worked by deep link. **Fix:** added `"mcp:reminder.list:call"`
to the member role next to `reminder.fire`. Requires a **node rebuild + restart** (Rust doesn't
hot-reload; re-login to re-mint the token with the new cap).

## What shipped (UI only — backend verbs unchanged)

- `RemindersStats.tsx` (new) — a record-derived KPI strip: total, active, paused, completed, summed
  firings (`runs`), and the soonest `nextAttemptTs` as a relative string. **No fabricated history
  feed** — see "past results" below.
- `ReminderDialog.tsx` (new) — the create **and** edit surface in one dialog (create upserts by id, so
  edit re-authors the same id with the name field locked). Reuses `CronBuilder` + `ActionEditor`.
- `RemindersView.tsx` (rewritten) — stats strip + "New reminder" toolbar + shadcn `Table` with
  per-row actions: **Run now** (`reminder.fire`), **Pause/Resume** + **Edit** (`reminder.update` /
  create-upsert), **Delete** (behind `ConfirmDestructive`). Empty state via `AppEmptyState`.
- `lib/reminders/reminders.api.ts` — added `fireReminder` (mirrors `reminder.fire`, camelCases
  `scheduled_ts`).
- `useReminders.ts` — added `fire(id)` returning the fire result so the view can surface a run-now
  **deny** inline (the documented dev-login `reminder.fire` re-resolve limitation —
  `docs/debugging/reminders/reminder-fire-reresolve-misses-token-caps.md`) rather than as a page error.

## "Past results from the DB" — deliberately NOT faked

Each firing is an internal `reminder-fire` **lb-job** (`host/src/reminder/fire.rs`), keyed
deterministically by `(reminder_id, scheduled_ts)`. There is **no `reminder.history`/list-firings MCP
verb** at v1, and `lb_jobs::pending` only returns resumable jobs (excludes terminal firing markers).
Rather than invent a fake data source (CLAUDE §9), the stats show the honest record-level facts —
notably the summed `runs` counter each firing advances. A true per-firing history table is a
**follow-up backend slice** (a new `reminder.history` verb querying the `job` table on
`data.kind = "reminder-fire"`); noted here so it isn't mistaken for done.

## Cron builder theming (follow-up fix)

Reported: the `react-js-cron` builder's dropdowns showed a mustard/brown selected+hover and didn't
match the app (wrong surfaces + radius). Root cause: `lib/widgets/inputs/CronBuilder.tsx` hardcoded
antd's `ConfigProvider` to `{ algorithm: darkAlgorithm, colorPrimary: "#f59e0b" }` — a fixed amber on
a fixed-dark base, ignoring the app's live theme (light/dark, custom accent), and never theming the
**portalled** dropdown menus. Fix: read the app's live CSS tokens off `:root`
(`--panel`/`--panel-2`/`--fg`/`--muted`/`--border`/`--accent`/`--accent-foreground`/`--muted-bg`) via
`getComputedStyle` and feed them to antd's token + `Select` component tokens (`optionSelectedBg`/
`optionActiveBg`/`optionSelectedColor` — validated names in antd 6.5); pick `defaultAlgorithm` vs
`darkAlgorithm` from the shell's `.dark` class; `getPopupContainer` anchors the dropdowns inside the
scoped subtree; `useThemeOptional()` re-renders on theme change (and degrades outside a provider).
antd still never touches the global theme (scope decision holds). `CronArg.test.tsx` (3/3) +
`RemindersView.gateway.test.tsx` (3/3) stay green.

## UI-standards conformance (`scope/frontend/ui-standards-scope.md`)

The page is held to the full standard (it is **not** in the ESLint `LEGACY_VIEWS` allowlist): the
canonical `<section className="flex h-full min-w-0 flex-col bg-bg text-fg">` wrapper leading with
`AppPageHeader` (icon + title + description + workspace chip; the "New reminder" button in its
`actions` slot), `Alert`/`AlertDescription` for the error (`destructive`) + notice banners, and the
body on shadcn `Table`/`Badge`/`Button`/`Dialog` + tokens only (no raw controls, no `globals.css`
control classes, no color literals). `pnpm eslint src/features/reminders` → 0 problems; `tsc` clean.

## Tests (real gateway, no mocks)

- `RemindersView.gateway.test.tsx` (updated for the dialog + confirm DOM) — create → list → pause/
  resume → delete(tombstone), all against the **real** spawned gateway + `reminder.*` host verbs +
  real store. **3/3 green** (`pnpm test:gateway RemindersView`).
- `RemindersStats.test.tsx` (new) — pure record-derived bucketing + summed firings + empty
  next-firing. **2/2 green**.
- Per-verb capability-deny + workspace-isolation remain proven server-side in the Rust integration
  tests (`reminders_mcp_test.rs`, `reminders_reactor_test.rs`) — unchanged.

## Notes for the next session

- The `ConfirmDestructive` confirm button's accessible name is its `aria-label="confirm action"`, which
  **overrides** the visible `confirmLabel` — target it by that label in tests, not by the button text.
- Run-now via a dev-login is denied by design at fire time; the view treats a `/denied/` error as an
  inline status, not a failure. Verify run-now happy-path against a durable grant (the Rust
  `reminder_fire_test.rs` already proves fire works when the action cap is granted durably).
