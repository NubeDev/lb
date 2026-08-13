# Dashboard scope — the relative time-range grammar (host slice)

Status: **IMPLEMENTED (unreleased — tag pending)**; session log:
`docs/sessions/dashboard/relative-time-range-session.md`. Owning repo: **this one (`lb`)**, released as **`node-v0.15.0`**; the
consumer slice (URL contract, picker, report CLI, page settings) lives in `NubeIO/rubix-ai` →
`docs/scope/frontend/dashboard/relative-time-range-scope.md`, which bumps the pin.

A dashboard window is stored and passed as two absolute ISO days, so a named window ("last 7 days") is
resolved once by whoever clicks it and then thrown away: a shared link freezes, a scheduled report has to
carry a private preset-id vocabulary, and nothing finer than a day is expressible. This slice gives the
platform **one relative-range grammar** — `today`, `yesterday`, `this-month`, `this-year`,
`last-3-months`, `now-4h` — with the canonical resolver in the host, a stored default range on the
dashboard record, and a schedule payload that names its window instead of freezing it.

## Goals

- `timerange`: parse + resolve the grammar against an injected clock and a timezone; one module, no
  callers reimplementing calendar maths.
- `Dashboard.time` — a typed, additive default window on the record, validated on save.
- `time.range.resolve` — a read-only host verb so flows, rules, agents and extensions can resolve a window
  without a private copy of the arithmetic.
- The report/reminder payload names its window with a range **expression** — the ONE vocabulary. (The
  legacy preset ids were dropped by decision, not carried; see build step 4.)
- A committed **conformance fixture** the downstream TypeScript twin asserts against, so the two
  implementations cannot drift silently.

## Non-goals

- No UI. The picker, the URL contract and the page-settings control are rubix-ai's.
- No per-panel time override (`timeFrom`/`timeShift`); the grammar makes it cheap later.
- No new record type, no new store table, no motion.

## Intent / approach

Resolution is pure arithmetic over `(expression, now, tz)`, so it belongs where it can be tested without a
browser or a network: the host. The shell cannot round-trip per navigation or per refresh tick, so it
carries a TypeScript twin — and the twin is held to the host's semantics by a generated fixture, not by
review. `chrono` + `chrono-tz` are already workspace deps; nothing new is added.

Rejected: **resolve only in the host** (a verb call per navigation) — a network hop in front of every range
change, and unusable offline for arithmetic. Rejected: **leave the maths downstream and keep the host
day-granular** — the reminder path already needs it (that is why `preset.rs` exists downstream), and a
second private vocabulary is exactly the drift this removes.

## The grammar

Endpoint expressions resolve to an instant; range tokens resolve to a whole window (legal in `from`, with
`to` absent).

- Endpoints: `now`, `now±<n><unit>` (`s m h d w M y`, Grafana-compatible — `m` minute, `M` month), an
  optional snap suffix `/<unit>` (`now-1d/d`), ISO day `yyyy-mm-dd`, ISO instant, 13-digit epoch ms.
- Range tokens: `today` / `yesterday` / `tomorrow`; `this-<unit>` / `last-<unit>` / `next-<unit>` over
  `hour day week month quarter year`; counted trailing windows `last-<n>-<unit>s` and the short
  `last-<n><unit>`.

Decided semantics (the ambiguities that made the existing two implementations disagree):

- `last-month` is the previous **whole calendar month**; `last-1-month` is a trailing month ending now.
- **The current period is "so far this period", not the whole period:** `today` and `this-<unit>`
  run from the START of the period to **now** — `today` = `[00:00 today, now)`, `this-week` =
  Monday → now, `this-year` = 1 Jan → now. `yesterday`/`tomorrow` and `last-<unit>`/`next-<unit>`
  stay whole periods.
- `to` is **exclusive**; a range token with a `to` is refused.
- Month/year arithmetic is calendar-aware (31 Mar − 1 month = 28 Feb).
- Weeks start **Monday**; quarters are Jan/Apr/Jul/Oct.
- Timezone precedence: `Dashboard.timezone` → the caller-supplied `tz` → UTC.

## How it fits

- **Capabilities & the deny path.** `time.range.resolve` is an ordinary host tool gated by
  `mcp:time.range.resolve:call`; a subject without it hits the caps wall and gets the standard refusal.
  No new write surface — the stored default rides `dashboard.save`'s existing gate.
- **Isolation.** `Dashboard.time` is a field on a `dashboard:{id}` record; per-workspace reads/writes are
  unchanged.
- **Rule 10.** The grammar names no extension and no board; the verb is registered generically and reached
  through ordinary tool resolution.
- **Symmetric nodes.** The clock is a parameter; no role branch.
- **One responsibility per file.** `timerange/{grammar.rs, civil.rs, resolve.rs, tool.rs, mod.rs}`, each
  well under 400 lines (grammar/resolve carry their unit tests in sibling `tests/` files to stay so).
- **No mocks.** Injected clock, real store, real caps wall in the host tests.

## The build

1. `crates/host/src/timerange/grammar.rs` — `parse(&str) -> Result<RangeExpr, TimeRangeError>`;
   `RangeExpr = Endpoint(..) | Window(..)`. Errors name the offending token and the legal set.
2. `crates/host/src/timerange/civil.rs` — the calendar maths (Hinnant `civil_from_days` /
   `days_from_civil`, ported from rubix-ai's `src/report/preset.rs`, plus month/quarter/year stepping).
3. `crates/host/src/timerange/resolve.rs` —
   `resolve(from, to: Option<&str>, now_ms: i64, tz: Tz) -> Result<ResolvedRange>` returning
   `{ from_ms, to_ms }` and the ISO-day projection the URL/report path uses.
4. ~~`crates/host/src/timerange/legacy.rs` — the seven shipped report preset ids mapped to expressions
   reproducing today's dates byte-for-byte.~~ **DROPPED BY DECISION (2026-08-03) — not an oversight.**
   It was built, then hard-deleted the same session: there is **no production deployment**, so no
   scheduled report exists whose window could shift on upgrade, and the only thing a compat layer
   would have bought is a second window vocabulary to keep in sync forever. The grammar is now the
   ONLY vocabulary — `last-7-days` is a grammar token meaning what the grammar says, not a preset id
   with its own arithmetic (note the two DID disagree: legacy `this-month` ended after *today*).
   The `preset` key is refused loudly wherever it appears (see 7), never aliased or ignored.
5. `crates/host/src/timerange/tool.rs` — the `time.range.resolve` descriptor + handler
   (`from`, optional `to`, `tz`, `now`) → `{ fromMs, toMs, fromIso, toIso }`.
6. `crates/host/src/dashboard/model.rs` — additive typed `DashboardTime { from: String, to: String }` as
   `Dashboard.time` (typed, because the `Dashboard` struct drops untyped page keys on save). All four
   layers, following the shipped `width` precedent: model field, `save.rs` JSON-schema entry +
   `dashboard_save_meta` `Option<_>` preserve-on-omit, `dashboard/tool.rs` arg mapping, gateway
   `POST /dashboards` field. Expressions are **validated on save**.
7. Report/reminder payload — `{"range":{"from":"last-month","to":…,"tz":…}}` is the **only** named-window
   form, validated at **save** time (a bad expression must fail with a human watching, not at 03:00
   nightly) and resolved at fire time. Per decision 4 above, a `{"preset":"…"}` key is **refused
   loudly at save AND at fire** — naming the dead key and its replacement — rather than accepted or
   silently ignored: a silently-ignored `preset` leaves a reminder that LOOKS configured and mails a
   fallback window nightly.
8. `docs/contracts/time-range-conformance.json` — 83 rows generated and asserted by a `timerange` test;
   rubix-ai vendors the file and asserts it from vitest. (No row ever pinned a legacy preset id — the
   fixture is pure grammar — so dropping the compat layer changed it not at all.)

## Example flow

1. A reminder is saved with `range.from = "last-month"`; `dashboard.save`/the schedule save parses it and
   refuses anything unresolvable.
2. On 1 September the outbox target fires; `resolve("last-month", None, now, Australia/Sydney)` returns
   1–31 August, and the renderer is handed concrete dates.
3. Meanwhile a flow needs "the last 6 hours" for an alert digest: it calls `time.range.resolve` with
   `from = "last-6-hours"` and gets the ms pair — the same arithmetic, no copy.

## Testing plan

- **Capability-deny** — `time.range.resolve` without `mcp:time.range.resolve:call` is refused at the caps
  wall (fresh subject + `authz.resolve`).
- **Workspace isolation** — extend the shipped cross-workspace dashboard test to cover `Dashboard.time`.
- **Offline/sync, hot-reload** — N/A (pure compute, no motion, no extension surface).
- **Conformance** — every token, both `last-month` spellings, 29 Feb 2028, 31 Mar `last-1-month`,
  1 Jan `last-month` across the year boundary, an `Australia/Sydney` DST transition, a Monday-start week.
- ~~**Legacy compat**~~ — dropped with the compat layer itself (build step 4: no production deployment).
  Replaced by a **removal** test: a `preset` key is refused at save AND at fire, in both payload carriers,
  with a message naming the dead key and its replacement.
- **Malformed input** — `last-fortnight`, `this-month` with a `to`, an empty string: refused with the bad
  token named; nothing defaults silently.
- **Save round-trip** — `dashboard.save` sets `time`, a later save omitting it preserves it, an invalid
  expression is refused and leaves the stored value untouched.

## Risks & hard problems

- Month-end and DST are where a date library gets quietly wrong; both are pinned by fixture rows rather
  than by reading the code.
- Two implementations (Rust + the downstream TS twin) is a deliberate cost — the fixture is the mitigation
  and the only artefact that must stay honest.
- ~~Moving live scheduled behaviour out of rubix-ai into the host~~ — a non-risk once the compat layer
  was dropped: nothing is deployed, so no live schedule can shift. The downstream `preset.rs` is deleted
  outright rather than mirrored.

## Open questions

None. Every ambiguity found while scoping is decided above: `last-month` vs `last-1-month`, the exclusive
`to`, the timezone precedence, Monday weeks, calendar quarters, and host-verb-not-hot-path. The one
decision REVERSED after scoping: legacy preset ids are **not** kept — with nothing in production there is
no window to preserve, so they were hard-deleted and the key is now refused (build step 4).

## Related

- Consumer: `NubeIO/rubix-ai` → `docs/scope/frontend/dashboard/relative-time-range-scope.md`.
- `docs/scope/dashboard/dashboard-kind-scope.md` — the record this field lands on.
- `docs/scope/viz/sql-time-macros-scope.md` — the query-time `$__timeFilter`/`$__timeGroup` macros the
  resolved window feeds.
