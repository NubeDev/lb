# Session — the Case plane (lb half)

- **Status:** in-progress
- **Started:** 2026-09-10
- **Branch:** `feat/case-plane` (worktree `~/code/rust/lb-cases-wt`)
- **Scope:** `docs/scope/insights/case-plane-scope.md` (this repo) — derived from the product scope
  in `NubeIO/rubix-ai → docs/scope/insights/case-plane-scope.md`.
- **Downstream:** `NubeIO/rubix-ai` `feat/case-plane` consumes this through a local `[patch]` until
  it is merged and a `node-v*` tag is cut.

## The ask

Build the upstream half of the Case plane: the `lb-cases` crate, the `case.*` verb surface, the
case-group / hold-down / sla-clock reactors, the `service_policy` + business-hours deadline
arithmetic, the contractor `case_request` round trip with its `GET /r/{token}` token principal, and
the changes the insight raise hot path takes (the `tags.*` dispatcher door, the `Human > Producer`
fold, `evidence.subjects[]`, the data-quality caveat stamp, `category` validation, `month_hist` +
`pattern`, the `case_id` echo).

## Baseline — captured, and GREEN

lb master is red on some jobs and some tests race under parallelism, so a failure that is not ours
is only provable against a baseline on the untouched branch. Captured on `origin/master`
(**`f8633b6d`**), in the primary `~/code/rust/lb` checkout (clean, at exactly that commit) rather
than a fresh worktree — see the disk incident below.

```
cargo test -p lb-insights --no-fail-fast
test result: ok. 3 passed; 0 failed …
test result: ok. 10 passed; 0 failed …
EXIT=0
```

```
cargo test -p lb-host --no-fail-fast \
  --test insights_test --test insight_triage_test --test insight_tag_echo_test \
  --test insight_evidence_test --test insight_analysis_test --test insight_assignee_notify_test \
  --test tags_test --test tags_isolation_test --test outbox_relay_ops_test \
  --test reminders_mcp_test --test reminders_reactor_test

insight_analysis_test          ok. 16 passed; 0 failed
insight_assignee_notify_test   ok. 16 passed; 0 failed
insight_evidence_test          ok. 10 passed; 0 failed
insight_tag_echo_test          ok. 11 passed; 0 failed
insight_triage_test            ok. 17 passed; 0 failed
insights_test                  ok. 22 passed; 0 failed
outbox_relay_ops_test          ok.  5 passed; 0 failed
reminders_mcp_test             ok.  6 passed; 0 failed
reminders_reactor_test         ok.  9 passed; 0 failed
tags_isolation_test            ok.  1 passed; 0 failed
tags_test                      ok.  3 passed; 0 failed
EXIT=0
```

**Every suite this session touches is green at the baseline**, so any failure from here is ours and
none of the "lb master is red" / "it races under parallelism" excuses apply to these eleven. What is
NOT baselined is the rest of `lb-host` (~100 test binaries) and the gateway — building all of them
a second time is what filled the disk (below), so the final report says plainly which suites were
compared and which were not, rather than implying a comparison that was never run.

### The disk incident (2026-09-10)

The first baseline attempt ran in a dedicated worktree with its own `target/`. Between the agents'
shared 213 GB target dir and that second 43 GB one, the box hit **100 % of 916 GB** and builds began
failing as `zigcc failed: signal: 6 (SIGABRT)` — a linker abort on one crate, with the honest
`No space left on device` landing on a *different* crate three lines later. All three agents were
messaged at once so none of them debugged a phantom compile error, the second target dir was
deleted (44 GB recovered), and the baseline was re-run in the primary checkout instead.
Written up in rubix-ai `docs/debugging/insights/second-target-dir-filled-the-disk.md`.

## Waves

| wave | slice | agent | state |
|---|---|---|---|
| 1 | raise-path changes: tags door, `Human > Producer` fold, backfill, `evidence.subjects`, caveats, `category` vocab, `month_hist`/`pattern`, `case_id` echo | insights-raise | in-progress |
| 1 | `lb-cases` crate, `case.*` verbs, case-group + hold-down reactors, triage migration | cases-core | in-progress |
| 1 | `service_policy`, the calendar type, the pure deadline arithmetic, `policy.sla.*` | sla-policy | in-progress |
| 2 | the sla-clock reactor | sla-clock | pending |
| 2 | `party`, `case_request`, the outbox link, the nudges, the gateway token principal | requests | pending |
| 2 | `rule.scorecard` | scorecard | pending |

## Decisions taken

Recorded as they are made — each with the alternative rejected and why. See also the scope doc's
*Resolved decisions*, which this list extends with anything decided during the build.

1. **The tag door keeps the shipped verb names** (`tags.add`/`tags.remove`/`tags.of`/`tags.find`),
   dispatched by EXACT name rather than a `tags.` prefix. The product scope calls the write
   `tags.set`; `tags.add` has shipped as an upsert on `(entity, tag, source)` since the tags scope,
   and minting a second name for one write forks the surface for a cosmetic reason. Dispatching by
   exact name (not a prefix) follows the `ext.list` precedent: the host does not reserve a whole
   namespace against a hypothetical extension called `tags`. *Rejected: `tags.set` as an alias* —
   two names for one write is how a catalog stops being readable.
2. **`mcp:tags.remove:call` joins the AUTHOR bundle with the door.** It exists in NO role bundle
   today, so a dispatcher entry alone would leave `tags.remove` `Denied` for every caller including
   admins — the shipped-but-unusable shape `tool_gate.rs` already documents four times. `tags.of`
   is aliased onto `tags.find` instead of being granted separately (reading one entity's tags is
   `tags.find` narrowed to one entity, not a second privilege).
3. **Grouping runs inline at the end of `insight_raise`, with a reconcile loop as the backstop.**
   The invariant is "every open insight is in exactly one open case"; a scan-only reactor makes that
   *eventually* true, so the queue under-reports for a tick and a UI that raises-then-lists sees a
   case-less row. Inline is where the tag echo and the subscription matcher already run. The loop
   (`spawn_case_reactors`) is the restart-safe backstop and doubles as the triage backfill.
   *Rejected: a scan-only reactor* (eventual consistency on a completeness invariant) and *a
   synchronous verb the UI calls* (two writers for one fact).

### Wave 2 (2026-09-10)

4. **The breach alarm assigns itself one cap, in one workspace.** `fire_reminder` re-resolves the
   stored `principal_sub`'s caps from the **durable grant store**, and a system actor holds none —
   so the alarm would have been a permanent, silent `denied=1`. It therefore does
   `grant_assign(Subject::User("system:sla-clock"), "mcp:case.breach:call")`: an ordinary grant row,
   visible in `authz.resolve`, revocable by an admin. Since `mcp:case.breach:call` is in **no role
   bundle**, that single grant is the only thing in the workspace holding it. *Rejected: widening
   `flows/reactor_loop.rs::reactor_caps()`* — that list mints a principal for the **flow** loop and
   the reminder fire path never reads it, so the change would have looked like a fix and done
   nothing. This is the [[green-while-broken-reactor-tests]] shape caught before it shipped: the
   RED half revokes the grant, drives the real reactor, and asserts `(fired, denied) == (0, 1)`.
5. **The breach reminder is constructed directly, not via `reminder_create`.** `reminder_create`
   derives `next_attempt_ts` from cron, which resolves only to the **minute**; `due_at` is an exact
   millisecond out of business-hours arithmetic. Rounding fires the alarm up to **59s early**, and
   declaring a breach before the deadline has passed is worse than being a minute late. So
   `next_attempt_ts = due_at.div_ceil(1000)` with `max_runs: Some(1)`. The `schedule` still carries
   a real 5-field cron naming that minute — **load-bearing, not decorative**: a denied firing takes
   `react.rs::reschedule` → `next_after(&schedule, now)`, and a `BadCron` there propagates out of
   `react_to_reminders` and aborts the **whole workspace's** reminder pass.
6. **`breached_ts` is `due_at`, never the firing instant.** A reactor that ticks late must not make
   a case look less late than it was. Set-once, and the `closed` check sits **after** the set-once
   guard so a case that breached and was later resolved keeps its breach.
7. **`EventKind::Sla` is a new kind, not folded into `workflow`.** A deadline is not a state
   transition, and `workflow` is the one stream a reader scans to reconstruct state. This is the
   only wave-1 record touched beyond registration. An **unmatched** case still writes one `sla`
   event stating the absence — found as a real bug, because an unmatched case is byte-identical
   before and after (all `None`), so the idempotence shortcut swallowed the statement and a reader
   could not tell "no contract governs this" from "the clock never ran".
8. **The scorecard reads unpaged, not through `lb_cases::list`.** `list` pages at
   `MAX_CASE_PAGE = 200`; a precision computed over the first 200 rows and rendered as *the rule's
   precision* is exactly the lie this slice exists to remove.
9. **A deleted primary insight is counted under `unknown`, never dropped.** Dropping it shrinks a
   denominator, so every surviving precision silently becomes a claim about a smaller population
   than the reader thinks they are seeing. *Rejected: skip with a `warn!`* — a log line nobody reads
   is not a disclosure. *Rejected: fail the verb* — one deleted insight must not take the scorecard
   down.
10. **`rule.scorecard` is registered by EXACT name, never as a `rule.` prefix.** `rules.` (the rules
    engine) is already a prefix; the two cannot shadow each other. Adding a `rule.` prefix would
    reserve a second, one-letter-different namespace against a hypothetical extension called
    `rule` — the mistake `ext.list`, `update.*`, `tags.*` and `policy.sla.*` all already avoid.
    *Rejected: renaming to `rules.scorecard`* — it would file a case-plane verb under the rules
    engine's owner and make the catalog read as if the engine computed it.

**A known N+1, stated rather than hidden.** `rule.scorecard` does one scan of `case` plus one point
read of the primary insight **per resolved case** (memoized per insight id). Acceptable for a rules
page, not for a hot path. The fix is an `origin_ref` echo written onto the case at open — the same
host-computed, self-healing discipline `case_id`, the owner echo and the tag echo already use —
which collapses it to one scan with zero reads. That is a wave-1 *record* change and wave 1 was
already committed, so it is named here and in the module docs rather than done.

## Test evidence

Pasted below as each wave lands.

## What is not done

Filled in at the end, explicitly.
