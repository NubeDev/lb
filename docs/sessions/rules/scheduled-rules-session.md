# Rules — scheduled rules (`#[schedule(...)]` → managed cron flow) — session

- Date: 2026-07-21
- Scope: ../../scope/rules/scheduled-rules-scope.md
- Sibling scope: ../../scope/flows/flow-trigger-schedule-authoring-scope.md (shared next-runs preview)
- Stage: S8 — data plane. See STATUS.md.
- Status: **in-progress → backend + tests GREEN.** Directive compile, the managed-flow syncer, the
  `rules.get`/`rules.list` read surface, and the end-to-end firing on the real `react_cron` reactor are
  all built and tested. Frontend (rule-page schedule block) not yet wired — see "Not done".

## Goal
Let a rule declare its own schedule with one line at the top of its body —
`#[schedule("every 15 minutes")]` — parsed (never executed) at save into a `croner` cron string, and
**compiled to a managed `cron → rule` flow** that the **existing** flow cron reactor fires. Self-
describing rule, no canvas, and still **exactly one scheduler**.

## The one architectural rule (held)
The directive is **authoring sugar compiled at save, never executed.** There is **no rule-cron
reactor** — the syncer builds an ordinary enabled `cron` flow and `react_to_flows_cron` fires it.
Ship gate: `scheduled_rules_test::no_rule_cron_reactor_exists` greps `crates/host/src` and fails if any
file reacts on a clock while reading rule schedules. It passes (no such reactor exists).

## What was built (by slice)

### Slice 1 — directive extract + NL→cron compile (`lb-rules`)
- **`crates/rules/src/schedule.rs`** — the `phrase → cron string` seam and nothing more:
  - `extract_schedule(body) -> Result<Option<RuleSchedule>, ScheduleError>` — a strict top-of-body scan
    (a line whose first non-space chars are `#[schedule`), so the token inside rule logic is never
    mistaken for the directive. One directive max (a second is `Malformed`).
  - `compile_phrase(phrase) -> Result<String, ScheduleError>` — a **vendored thin phrase-matcher** for
    the common phrases (`every N minutes/hours`, `hourly`, `daily`, `weekdays at HH:MM`, `… at HH:MM`
    with am/pm). Explicit `#[schedule(cron = "...")]` passes through. Emits ONLY a cron string — never a
    time. UTC v1.
  - `strip_directive(body) -> Cow<str>` — removes the directive line before the cage compiles the body
    (added while fixing the run-time bug below); zero-copy on the unscheduled path.
- **`SavedRule.schedule: Option<RuleSchedule>`** (`{raw, cron}`) — additive serde default.
- **`rules_save`** now extracts + compiles the directive (before the write, body still not executed) and
  **croner-validates** the emitted cron via `lb_reminders::is_valid` — an unparseable phrase or an
  invalid compiled cron is a **save error** (`BadInput`), never a silent no-schedule. It returns
  `(id, Option<RuleSchedule>)` so the bridge can run the syncer.

### Slice 2 — the managed-flow syncer (`host`)
- **`crates/host/src/rules/schedule.rs`** — `sync_schedule(...)`:
  - `Some(sched)` → create `flow:{ws}:schedule:{rule_id}` (id `schedule:{rule_id}`), two nodes
    `cron trigger (config.cron) → rule node (config.rule)`, `enabled`, `start_on_boot`,
    `managed_by = "rule-schedule:{rule_id}"`. On re-save, diff the trigger cron and issue **one**
    `flows.node.update` only if it changed (idempotent re-save = no-op, no version bump).
  - `None` → `flows.delete` the managed flow (directive removed → run-on-demand).
  - Goes through the **existing** `flows_save`/`flows_node_update`/`flows_delete` verbs under the **same
    caller** — scheduling = `rule-write ∩ flow-write`, **no widening**. A flow-write deny surfaces as
    `{managed:false, pending:"needs flow-write"}` — the rule + its schedule metadata already persisted,
    so the contract is explicit (never told "scheduled" when nothing fires).
- **`Flow.managed_by: Option<String>`** — additive serde default marking derived-state flows.

### Slice 3 — read surface (`host`)
- **`rules.get`** gains a resolved `schedule` block `{raw, cron, next_runs, flow_id, managed, drift?}`.
  `next_runs` are the next 5 firings via `lb_reminders::next_after` — the **same croner engine the
  reactor fires on**, so the preview never lies. `drift = true` when the managed flow's trigger cron was
  hand-edited away from the directive (allow-and-flag; the next save re-asserts).
- **`rules.list {scheduled:true}`** — the roll-up: only rules carrying a compiled schedule.

### Slice 4 — firing end-to-end (real `react_cron`)
- No new firing code. The managed flow is an ordinary enabled cron flow; `react_to_flows_cron` primes
  its cursor, fires exactly one run per due instant, the `rule` node runs the saved rule via
  `rules.eval`, and the insight is raised + dedups on the second tick.

## The bug this session found + fixed
`debugging/rules/scheduled-rule-directive-breaks-cage.md`: the directive is stored **in the body**, but
`#` is reserved in rhai, so running the rule (`rules.eval` loading the stored body) errored
`'#' is a reserved symbol`. Invisible through slices 1–3 (parse-at-save never runs the body); only the
slice-4 firing test exposed it. Fixed with `strip_directive` at the single cage-compile chokepoint
(`engine.rs`), leaving the stored body (the schedule source of truth) intact. Regression: a `lb-rules`
unit test + the e2e firing test (fails-before/passes-after).

## Open questions — resolved (per scope recommendations)
1. Syntax → **attribute-style `#[schedule(...)]`**.
2. NL parser → **vendored thin phrase-matcher**, NOT `natural-cron`. Verdict: `natural-cron` is MIT
   (license OK) but a `0.0.2`, ~17%-documented crate whose API is a cron *builder/validator*, not a
   `phrase → cron` parser — too immature for a core crate (rule 1). The seam is swappable to `ai.*`.
   `croner` (in-tree, MIT) stays the ONE time engine.
3. Managed-flow edit policy → **allow-and-flag drift**; re-assert on save.
4. Second door (`rules.schedule.set`) → **deferred** (the directive is the chosen v1 door).
5. Timezone → **UTC v1, documented**.

## Not done (explicit gaps, not silent)
- **Frontend** (rule-page schedule block + next-5 preview + `scheduled:true` list filter UI) — the
  backend contract (`rules.get.schedule`, `rules.list {scheduled}`) is complete and tested; the React
  surface lives in a product host (`packages/*` / rubix-ai), not this library, and is a fast-follow.
  The shared `(cron, now) → next-5` **fixture** is asserted here against `next_after`; the frontend
  helper must assert the identical vectors (the parity guard the sibling scope names).

## Tests (all green — see output below)
- `crates/rules/src/schedule.rs` unit: 10 (phrase→cron, explicit passthrough, unparseable = error,
  double/empty/malformed directive, 12am/pm edges, strip-before-cage regression).
- `crates/host/tests/scheduled_rules_test.rs`: 12 (real store/caps/bridge/reactor, no mocks):
  directive compiles on save; unparseable = save error; managed flow built (2 nodes + cron + marker);
  idempotent re-save → update → delete reconcile; **capability-deny** (no cap; **split-grant** rule-
  write-no-flow-write → pending); **workspace-isolation** (ws-B can't read/build/fire a ws-A schedule);
  read block + **preview parity** vs `next_after`; list filter; drift flag + re-assert; **firing e2e**
  (one run → insight → dedup); the **no-rule-cron-reactor ship gate**.

```
running 12 tests
test no_rule_cron_reactor_exists ... ok
test unparseable_directive_is_a_save_error ... ok
test ws_b_cannot_read_or_build_a_ws_a_managed_flow ... ok
test save_builds_the_managed_flow ... ok
test split_grant_persists_schedule_but_reports_pending ... ok
test list_scheduled_filter_returns_only_scheduled_rules ... ok
test directive_compiles_to_cron_on_save ... ok
test drift_is_flagged_when_the_managed_flow_is_hand_edited ... ok
test scheduled_rule_fires_through_the_flow_cron_reactor_and_dedups ... ok
test resave_is_idempotent_then_updates_then_deletes ... ok
test rules_save_denied_without_the_cap ... ok
test rules_get_carries_the_schedule_block_and_next_runs ... ok
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.45s

running 10 tests (lb-rules schedule::tests)
... common_phrases_compile / twelve_am_pm_edges / unparseable_* / extract_* /
    directive_line_is_stripped_before_the_cage / double_directive_is_malformed / ...
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 80 filtered out
```
`cargo build --workspace` green. `cargo test -p lb-rules -p lb-flows` green (96 + others). The only
workspace test failure is a **pre-existing, unrelated** `lb-cli` fixture (`include_bytes!` a
`hello_v2_ext.wasm` not built in this env) — untouched by this change.

## Cross-links
- Scope: `scope/rules/scheduled-rules-scope.md` (open questions resolved).
- Debug: `debugging/rules/scheduled-rule-directive-breaks-cage.md`.
- Public: `doc-site/content/public/rules/rules.md` (schedule section).
- Skill: `skills/rules/SKILL.md` (schedule-a-rule walkthrough).

---

# Addendum — 2026-08-14: scheduled rules were denied on every fire (lb#167)

Slice 4 above shipped a green end-to-end firing test, and scheduled rules **still never worked on a
running node**. The follow-up session that closed [#167](https://github.com/NubeDev/lb/issues/167).

## What was wrong

A live probe (`#[schedule("every 1 minute")]`) saved, compiled `*/1 * * * *`, built the managed flow,
and fired dead on the minute — with `{"id":"rule","outcome":"err","error":"denied"}` every time and
the run settled `partialFailure`. Six consecutive fires, all identical. `rules.run` on the same rule
succeeded.

That asymmetry is the whole bug: a **manual** run rides the caller's token; a **scheduled** fire runs
under the fixed system principal `node:reactor`, minted by `spawn_flow_reactors` from
`reactor_caps()`. Two different authorities — and `reactor_caps()` had no `rules.*` at all. (`mcp:
*.call:call`, already in the list, does not cover it: the mcp resource splits on `.`, so that pattern
means "second segment is literally `call`".)

The deny then **moved** as it was fixed, which is why this took two rounds:

1. `mcp:rules.eval:call` — the `rule` node's dispatch. Without it nothing runs. *(shipped in #168)*
2. `mcp:insight.raise:call` + `ack`/`resolve` — a raising rule is the ordinary FDD/EMS shape; raising
   carries a lifecycle, so `raise` without the ageing pair is a half-grant. *(shipped in #168)*
3. `mcp:inbox.record:call` + `mcp:outbox.enqueue:call` — the **other** finishing move, `alert(...)`.
   `rules::run::route_alerts` fans every alert-marked finding to inbox + outbox at the *end* of a
   successful eval, under the same principal, and a deny there fails the whole `rules.eval`. So an
   alerting rule stayed `denied` with 1 and 2 granted. *(this session)*

Named verbs throughout — the deliberate boundary in that function (blanket `mcp:*.*:call` refused for
`ext-call`, a fixed system principal carries no third-party reach) is unchanged. None of these leave
the workspace: an outbox effect is *enqueued*, and delivering it is the outbox worker's own
principal's job.

## Also fixed: `rules.delete` orphaned the managed cron

`rules.delete` tombstoned the rule record and left `flow:{ws}:schedule:{id}` running — a cron firing
forever at a rule that no longer exists, with no owner left to remove it through the rules surface.
It now runs the **same** teardown a directive-less re-save runs (`schedule::sync_schedule(.., None)`),
so there is one derived-state reconciler rather than two: idempotent (an absent flow is a no-op,
which also self-heals an already-orphaned one), and a caller without flow-write gets the honest
`pending` marker rather than a failed delete, exactly as on save. The delete response now carries the
`schedule` block.

## Why the green suite meant nothing

`scheduled_rules_test.rs` drove the managed flow with the author's **FULL** principal, which holds
every one of these caps. It exercised the reactor's *code path* while substituting a different
*authority* — the single variable the bug lived in. The suite was green through all six failing
fires.

The regression tests therefore mint the **production** principal from the production function:
`Principal::routed("node:reactor", ws, reactor_caps())` (`reactor_caps()` is `pub` for exactly this).
A hand-copied cap list in a test is a mirror that goes stale silently and turns green precisely when
the product breaks.

## Tests (green; both revert-checked)

- `scheduled_rule_fires_under_the_reactors_own_principal_not_the_authors` *(#168)* — rule step not
  `denied`, run `success`, insight landed.
- `scheduled_alerting_rule_routes_to_inbox_under_the_reactors_own_principal` — same for an `alert()`
  body; asserts the routed inbox item on the `rules` channel. Fails `left: "denied"` with the two
  caps removed.
- `rules_delete_tears_down_the_managed_flow` — after the delete `flows.get` is gone AND a far-future
  reactor tick fires **nothing** (the effect, not just the record); a repeat delete is a no-op. Fails
  on the surviving flow with the teardown removed.

```
running 15 tests
test scheduled_alerting_rule_routes_to_inbox_under_the_reactors_own_principal ... ok
test rules_delete_tears_down_the_managed_flow ... ok
test scheduled_rule_fires_under_the_reactors_own_principal_not_the_authors ... ok
... (12 more)
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.43s
```

`versions_test` (14), `rules_workflow_convergence_test` (9), `flows_run_test` (49),
`flows_triggers_test` (19), `flows_orphan_sweep_test` (3) all green. `rules_test` (3) and
`flows_platform_nodes_test` (4) have failures that reproduce **identically on a stashed clean tree**
— pre-existing, untouched by this change.

## Cross-links
- Debug: `debugging/rules/scheduled-rule-denied-under-reactor-principal.md`.
- Issue: lb#167. First half: PR #168.
