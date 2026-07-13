# Calendar scope — workspace / team / user calendars with shareable events

Status: scope (the ask). Promotes to `doc-site/content/public/calendar/` once shipped.

A Gmail/Google-Calendar-style calendar for the platform: every member has a personal
calendar, teams and the workspace have shared ones, and events can be shared — invite
members, they accept/decline, everyone sees the result, reminders fire before the event.
The platform already has almost all of the substrate (records + sharing edges + teams,
entity-scoped grants, the reminder/jobs scheduler, outbox + push delivery, federated UI);
the genuinely new surface is the calendar domain itself: events, RFC 5545 recurrence,
invites (attendee state), and `.ics` interop.

## Goals

- **Three calendar kinds, one model:** user (private by default), team (visible to team
  members), workspace (visible to every member). A member's day/week/month view merges
  every calendar they can reach.
- **Events with real recurrence:** one-off and recurring events (RFC 5545 RRULE —
  "every 2nd Tuesday", exceptions, all-day, timezone-correct across DST), with
  per-occurrence edits ("move just this Friday's standup").
- **Sharing & invites:** share a calendar with a user/team; invite members to an event
  and track accept / decline / tentative per attendee (RFC 5546 semantics).
- **Reminders/notifications:** "10 min before" per event, delivered via the existing
  channel-post / push machinery — no new scheduler.
- **Interop:** import and export `.ics`, so external calendars (Google, Outlook) can be
  brought in and events can be sent out.
- **Agent-drivable:** the workspace agent can read availability and create/move events
  through the same MCP verbs the UI uses.

## Non-goals (v1 defer-list)

- **Email (iMIP) invite delivery to external addresses** — v1 invites are workspace
  members only; external people get a `.ics` export. (When wanted, it composes:
  outbox email target + `icalendar` `METHOD:REQUEST` — and `auth-caps/invites-scope.md`
  already covers onboarding a not-yet-user.)
- **Free/busy queries and scheduling assistant** ("find a slot for these 5 people") — a
  natural v2 verb (`calendar.freebusy`) once occurrence materialization exists.
- **CalDAV server/client sync** — `.ics` file import/export only; live two-way sync with
  Google/Outlook is its own scope.
- **Resource booking** (rooms, equipment) — the model leaves space for it (an attendee
  whose subject is a resource), but no v1 surface.
- **Meeting links / conferencing integration.**

## Intent / approach

**One `calendar` extension** (native Tier-2), following the `cc-app` pattern: the
extension owns the domain records and runs its own authz chokepoint over the shipped
`authz.check_scoped` / `authz.scope_filter` / delegated-reach host callbacks
(sdk-v0.4.0). Core `lb` stays calendar-ignorant (rule 10) — the extension reaches the
platform only through the generic seams (store tables, `grants.*` over MCP dispatch,
jobs, outbox, extension-watch, UI federation).

Key design points:

1. **Records, not blobs.** Four SCHEMAFULL tables (typed, indexed, `store.query`-able —
   the nav/dashboard "not a JSON blob" precedent): `cal_calendar`, `cal_event`,
   `cal_occurrence`, `cal_attendee`.
2. **Canonical event + materialized occurrences.** An event stores its rule
   (`dtstart` + tzid + RRULE string + EXDATEs + per-occurrence overrides keyed by
   `RECURRENCE-ID`); a bounded window of concrete occurrences (rolling **past 6 / future
   18 months**) is expanded via the `rrule` crate into `cal_occurrence` (indexed
   `start_ts`), by a durable job on event write and a monthly top-up tick. Range reads
   are then a plain indexed scan — **never expand RRULEs at query time** (unbounded
   RRULE expansion on read is the classic calendar DoS; `rrule`'s validation limits stay
   on for user-supplied rules).
3. **Reach = grants for calendars, edges for invites.** Calendar visibility is an
   **entity-scoped grant** on `cal_calendar` (`{table:"cal_calendar", ids:[...]}`),
   minted by the extension via `grants.assign` when a calendar is created or shared —
   user calendar → grant to `user:{sub}`, team calendar → `team:{id}`, workspace
   calendar → `role:member`. Event reach **derives from the parent calendar**, plus a
   live `cal_attendee` check so an invitee sees an event even when they can't see its
   calendar. *Alternative rejected:* modeling calendars as shell assets with `share`
   edges — asset visibility is a core shell mechanism for shell-known asset types, and a
   calendar is extension domain surface; entity-scoped grants exist precisely for this
   (and `access-model-scope.md` already rejected team-per-entity for domain records).
4. **Recurrence engine = `rrule`, interop = `icalendar`.** `rrule` 0.14 is the only
   production-grade RFC 5545 expansion engine in Rust (MIT/Apache-2.0; chrono/chrono-tz
   stay internal to the extension). `icalendar` 0.17 (actively maintained, integrates
   with `rrule`) is the `.ics` codec at the edge — canonical truth is always the native
   records, `.ics` is import/export only. *Alternatives rejected:* `calico`
   (eikopf/calendar-crates — most spec-complete parser incl. iTIP, but pre-1.0,
   single-maintainer, tiny adoption; reconsider if v2 needs full iTIP parsing) and
   hand-rolling recurrence (DST + BYxxx interactions are a bug farm). Cron (`croner`)
   stays what it is — the *action* scheduler for reminders — it cannot express event
   recurrence semantics.
5. **Reminders reuse the scheduler we have.** A reminder for an occurrence is an
   `lb-jobs` job with `run_at = occurrence.start_ts - offset` and a deterministic id
   `(event_id, occurrence_ts, offset)` (idempotent, the reminders-scope pattern), whose
   firing writes an outbox effect (push target and/or channel post). No fourth
   scheduler (§ "no second scheduler" doctrine).
6. **Invites are a small state machine.** `cal_attendee {event, subject, partstat:
   needs-action|accepted|declined|tentative, sequence}` per RFC 5546: organizer edits
   that change time bump the event `sequence` and reset attendee `partstat` to
   needs-action; a reply carries the sequence it answers so stale replies are rejected.
   Invite/update/cancel notifications are must-deliver outbox effects.

## How it fits the core

- **Tenancy / isolation:** all four tables live in the workspace namespace (structural
  isolation); every verb takes ws from the token. Nothing crosses workspaces — a
  "calendar shared across workspaces" is out of scope.
- **Capabilities:** each verb is its own cap (`mcp:calendar.<tool>:call`); table access
  via `store:cal_*:read|write` requested in the manifest. Within the workspace,
  per-calendar reach is the entity-scoped grant; the extension chokepoint calls
  `authz.check_scoped` (single record) / `authz.scope_filter` (list — push returned ids
  into the indexed query) on **every** read/write, and uses **delegated reach**
  (`subject` param, gated by `mcp:authz.delegate_reach:call`) when the organizer's
  action must be evaluated against an invitee ("can X see this event?") — fail-closed.
  Deny path: no grant on the calendar and no attendee row → 403 for get, silently
  filtered from lists (same shape as nav cap-stripping). Note the shipped **freshness
  asymmetry**: revoking a calendar grant is TTL-stale until token re-mint; the
  attendee-edge check is live.
- **Placement:** either. Pure records + jobs; a solo edge node runs the whole thing
  offline. No `if cloud`.
- **MCP surface** (API shape per SCOPE-WRITTING §6.1):
  - *CRUD:* `calendar.create|update|delete`, `calendar.event.create|update|delete`
    (event update takes an optional `recurrence_id` for this-occurrence-only edits, and
    an `apply_to: one|following|all` selector), `calendar.share|unshare {calendar,
    subject, rw}` (mints/revokes the entity-scoped grant via `grants.assign|revoke` —
    depends on the `authz-verbs-mcp-dispatch` slice), `calendar.event.respond {event,
    partstat, sequence}`.
  - *Get / list:* `calendar.get`, `calendar.list` (calendars the caller reaches),
    `calendar.event.get`, `calendar.events.range {calendars?, from, to, cursor, limit}`
    — the merged-view read over `cal_occurrence`, keyset cursor + server-capped limit
    (offset paging is rejected platform-wide).
  - *Live feed:* `calendar.watch` — a `[[tools]] kind="watch"` feed (extension-watch →
    unified SSE) emitting record-change events for reachable calendars, so open views
    update without polling.
  - *Batch:* `calendar.ics.import {calendar, file}` — **a job** (returns job id;
    per-item results, partial failure allowed; caller watches via `job.watch`).
    `calendar.ics.export {calendar}` is synchronous (bounded: one calendar's canonical
    events, not occurrences).
- **Data (SurrealDB):** `cal_calendar {name, kind:user|team|workspace, owner_subject,
  color, default_reminder_offsets}`, `cal_event {calendar, title, body, location,
  dtstart_ts, dtend_ts, tzid, all_day, rrule?, exdates[], overrides{recurrence_id →
  patch}, sequence, organizer_sub}`, `cal_occurrence {event, calendar, start_ts, end_ts,
  recurrence_id}` (derived — rebuildable, indexed on `(calendar, start_ts)`),
  `cal_attendee {event, subject, partstat, sequence, comment?}`. Times are **epoch
  seconds** (the platform clock convention) + `tzid` for recurrence math; all-day events
  store floating dates. State only — nothing durable rides the bus.
- **Bus (Zenoh):** motion only — the watch feed's change events (fire-and-forget; a
  missed frame is healed by the next `events.range` read) on a host-allocated ws
  subject. Invite/cancel/reminder notifications are **must-deliver** and therefore go
  through the **outbox** (push target / channel post), not raw pub/sub.
- **Sync / authority:** plain workspace records — standard record sync; `cal_occurrence`
  is derived so sync conflicts on it are irrelevant (rebuild wins). Offline edge: create
  events offline, materialization job runs locally, effects drain on reconnect.
- **Secrets:** none in v1 (no external providers).
- **Stateless extension:** the sidecar holds nothing durable; expansion windows and
  attendee state are records — hot-reload/restart safe. Clock is the injected logical
  clock, never wall-clock.
- **SDK/WIT impact:** none — everything rides shipped seams (sdk-v0.4.0 host callbacks,
  `grants.*` MCP dispatch, extension-watch, jobs, outbox). Flag: if `grants.*`-over-MCP
  (`authz-verbs-mcp-dispatch`) hasn't merged when the build starts, `calendar.share`
  is blocked on it — it's the write-half dependency.
- **Skill doc:** yes — this is an agent-drivable surface. The implementing session
  writes and maintains `skills/calendar/SKILL.md` (create/move/respond/range-read,
  grounded in a live run).

## Example flow — invite a teammate to a recurring event

1. Ada (member) calls `calendar.event.create` on her user calendar: "Standup",
   `dtstart` Mon 09:00 `Australia/Sydney`, `RRULE:FREQ=WEEKLY;BYDAY=MO,WE,FR`,
   reminder offset 10 min, attendee `user:bob`.
2. The extension chokepoint checks `authz.check_scoped(ada, mcp:calendar.event.create,
   cal_calendar:ada)` → allow. Event row written; a durable job expands the RRULE via
   `rrule` into `cal_occurrence` rows for the 18-month window; reminder jobs are
   enqueued with deterministic ids; an **outbox effect** (invite notification → Bob's
   push/channel) is written in the same transaction as the attendee row.
3. Bob's shell shows the invite; he calls `calendar.event.respond {partstat: accepted,
   sequence: 0}`. His attendee row flips; Ada's open calendar view updates via
   `calendar.watch`.
4. Bob's `calendar.events.range` (merged view) now includes the standup even though he
   has no grant on Ada's calendar — the attendee edge grants event-level reach.
5. Ada moves Wednesday's occurrence only: `calendar.event.update {recurrence_id: …,
   apply_to: one}` — an override patch on the event, the one occurrence row rewritten,
   `sequence` bumped, Bob's `partstat` reset to needs-action, update effect delivered.
6. Friday 08:50 Sydney time: the reminder job fires (logical clock), writes the outbox
   push effect; Bob's phone buzzes.
7. Carol (different workspace) calls `calendar.event.get` on the event id → 403
   isolation gate; Dave (same workspace, no grant, not an attendee) → 403 deny.

## Testing plan

Per `scope/testing/testing-scope.md` — real store/bus/gateway, seeded real records, no
fakes (`rrule`/`icalendar` are in-process libraries, nothing to fake):

- **Capability deny (mandatory):** each verb without its cap → refused; `share` by a
  caller not holding the calendar → no-widening refusal; delegated-reach `subject`
  without the marker cap → 403 (never falls back to caller reach).
- **Workspace isolation (mandatory):** ws-B token cannot get/range/watch/respond on
  ws-A calendars/events, on both the store and MCP surfaces.
- **Entity-scope deny:** member without a grant and without an attendee row → event get
  403, range-filtered out; empty scope → empty list, not error.
- **Offline/sync (mandatory):** create/edit offline on an edge node → occurrences
  materialize locally; on reconnect, invite effects drain idempotently
  (`idempotency_key` dedup).
- **Recurrence determinism (injected clock):** weekly/monthly/BYDAY expansion snapshots;
  a **DST-boundary** case (09:00 Sydney across the April/October transitions keeps
  local time); EXDATE removal; `apply_to: one|following|all` edits; window top-up tick
  extends without duplicating (deterministic occurrence ids).
- **Invite state machine:** respond flips partstat; time-changing edit bumps sequence +
  resets partstat; stale-sequence reply rejected; cancel delivers must-deliver effect.
- **Reminder firing:** job fires at `start_ts - offset` on the logical clock; missed
  window while node down → fires once, no backfill storm (reminders-scope policy).
- **`.ics` round-trip:** export → import into a second calendar reproduces events
  (incl. RRULE + EXDATE); malformed `.ics` import → per-item errors in the job result,
  valid items land; hostile RRULE (e.g. `FREQ=SECONDLY` unbounded) → rejected by
  validation limits, not expanded.
- **E2E (UI):** month/week view renders a seeded recurring event over the real gateway;
  respond-from-invite updates both parties' views via the watch feed.

## Risks & hard problems

- **Recurrence correctness is the whole feature.** Timezone-anchored RRULEs across DST,
  `apply_to: following` splits, and override/EXDATE interaction are where calendars rot.
  Mitigation: never hand-roll (all expansion through `rrule`), snapshot-test expansion,
  keep occurrences derived/rebuildable.
- **Materialization drift.** Occurrence rows are a cache of the rule; a missed rebuild
  (crashed job) shows users a wrong calendar. Every event edit re-materializes in the
  same job; a heal pass re-expands on window top-up; `cal_occurrence` must always be
  safe to rebuild from scratch.
- **Grant-mint fan-out.** Share-to-team is one grant to `team:{id}` (inheritance is
  computed at mint) — resist the temptation to fan out per-member grants, which is the
  rejected team-per-entity explosion.
- **Revocation staleness.** Unsharing a calendar leaves the grant in issued tokens until
  re-mint (platform-wide asymmetry). Acceptable for v1; state it in the public doc.
- **Write-half dependency.** `calendar.share` needs `grants.assign|revoke` callable from
  the sidecar (`authz-verbs-mcp-dispatch`) — verify shipped before the build starts.
- **Volume.** 18-month windows of a busy workspace = tens of thousands of occurrence
  rows; that's fine for an indexed range scan, but the import job and top-up tick must
  batch writes (`write_batch`) and page, never load a calendar into memory.

## Open questions — RESOLVED (v1 shipped 2026-07-13, `lb-extensions/extensions/calendar`)

1. **Merged-view default → RESOLVED: all reachable.** `calendar.events.range` with no `calendars`
   filter returns all reachable calendars (one `scope_filter` call) PLUS events the caller is a live
   attendee of. An explicit `calendars` list is a scoped read (no attendee widening).
2. **Event-level reach for attendees → RESOLVED: live edge check.** The chokepoint checks the
   `cal_attendee` edge live (both single `event.get` and the merged `events.range`), NOT a per-event
   grant. (A regression bug where the merged range missed the edge was caught by the live run + fixed.)
3. **`apply_to: following` → RESOLVED: second-event split (Google's model).** Cap the original rule
   with `UNTIL` at the split; create a `{id}-fwd-{ts}` event carrying the edited tail.
4. **Default reminder offsets → RESOLVED: calendar record + per-event override, no prefs axis in v1.**
   `cal_calendar.default_reminder_offsets`, overridable per event via `reminder_offsets`.
5. **Placement → RESOLVED: out-of-tree in `lb-extensions`.** Standalone project, production graph pins
   only the published SDK (`lb-ext-native`, sdk-v0.4.0); no `lb` deps. (Real-node tests use a dev-only
   git dep on lb until a published `lb-ext-testkit` exists.)
6. **Agenda widget → RESOLVED: v1 (blueprint).** Specified in `ui/README.md`; buildable once
   `@nube/ext-ui-sdk` publishes (unpublished this session — Rule 9 forbids faking it).

### New findings (platform gaps surfaced building v1 — follow-ups, not calendar-specific)

- **No extension-facing SCHEMAFULL/DEFINE-INDEX seam.** The four tables ride the generic
  `store.write`/`store.query` verbs as SCHEMALESS records (typed structs, `store.query`-able, bounded
  range scan) — "SCHEMAFULL + indexed" is met functionally, not literally. A published
  `store.define_schema` / migration hook would close the gap.
- **No native extension-watch seam** → `calendar.watch` (live feed) is declared but not wired; poll
  `events.range` (a missed frame heals on the next read).
- **No `jobs.*` callback seam** → `ics.import` runs inline (per-item results) rather than as a durable
  job.
- **`store.query` rejects `ORDER BY`/`LIMIT`** inside its bounding wrapper → `events.range` sorts +
  keyset-pages in Rust over the window-bounded set (fine for v1; a bounded server-side ORDER BY would
  let it page past the host row cap in one span).

## Related

- `README.md` §3 (rules), §6.10 (jobs); `docs/SCOPE-WRITTING.md` §6.1 (API shape).
- `scope/auth-caps/entity-scoped-grants-scope.md` (per-calendar reach),
  `scope/auth-caps/native-caller-identity-scope.md` (delegated reach),
  `scope/auth-caps/authz-grants-scope.md` (grants/teams), `scope/auth-caps/invites-scope.md`
  (future external invitees).
- `scope/reminders/reminders-scope.md` + `scope/jobs/jobs-scope.md` (scheduling),
  `scope/inbox-outbox/outbox-scope.md` + `push-target-scope.md` (delivery).
- `scope/extensions/extensions-scope.md`, `ui-federation-scope.md`,
  `extension-watch-scope.md`; `rust/extensions/ros/` (data-heavy ext reference).
- Crates: [`rrule`](https://github.com/fmeringdal/rust-rrule) (expansion),
  [`icalendar`](https://github.com/hoodie/icalendar) (.ics codec); rejected:
  [`calico`](https://github.com/eikopf/calendar-crates) (pre-1.0, revisit for iTIP).
- `skills/calendar/SKILL.md` — written by the implementing session (drivable surface).
- `doc-site/content/public/calendar/calendar.md` — stub, filled on ship.
