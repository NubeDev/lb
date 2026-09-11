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

Full re-verification of the branch at its tip (2026-09-11), in this worktree, against
`origin/master` base `f8633b6d`.

```
cargo fmt --all --check                    clean
cargo clippy --all-targets                 0 errors
```

15 files carry a clippy **warning**; none of them is one of the branch's 146 changed `.rs` files
(set-intersected, not eyeballed). Every warning is pre-existing on master: `ext-loader/manifest.rs`,
`host/src/{ext/versions,federation/update,nav/reach,outbox/relay_ops,report/compose}.rs`,
`ingest/src/decode/nem12.rs`, `mcp/src/call/dispatch.rs`, and seven test files.

**The case plane's own suites — all green:**

| suite | tests |
|---|---|
| `lb-cases` lib | 34 |
| `lb-cases` `deadline_test` | 17 |
| `lb-cases` `policy_store_test` | 7 |
| `case_suite` (all eight case-plane suites, one binary) | **86** |

`case_suite` is the aggregate of what were eight top-level test files — plane 15, request 18,
reactor 9, sla_clock 9, sla_policy 11, scorecard 10, caveat 8, delegation 6 — now split by concern
under `crates/host/tests/case/` and declared as modules of ONE harness, the way `agent_suite.rs`
already does for the 29 agent files. Cargo links the whole dependency graph into every top-level
`tests/*.rs`, so this is also ~7 GB less `target/`.

**The surfaces the branch touches — all green:** `lb-host --lib` **582**; `tags_test` 3,
`tags_door_test` 5, `tags_isolation_test` 1; `insights_test` 22, `insight_tag_echo_test` 11,
`insight_tag_fold_test` 4, `insight_evidence_test` 10, `insight_triage_test` 17,
`insight_analysis_test` 16, `insight_assignee_notify_test` 16; `reminders_mcp_test` 6,
`reminders_reactor_test` 9, `reminder_fire_test` 8, `pack_reminders_test` 5;
`email_transport_test` 6, `invite_email_relay_test` 1, `mail_suite` 11.

**The whole gateway role: 64 suites, 393 tests, 0 failures.** Two need fixtures built once or they
read as failures rather than as a missing build:

```bash
cargo build -p echo-sidecar
cd extensions/hello-v2 && cargo build --release --target wasm32-wasip2
```

One `cargo test -p lb-role-gateway` run died with `error: linking with \`cc\` failed` and passed
completely on an immediate re-run with nothing changed — contention with another cargo process in the
same target dir, not a code fault. Worth knowing before it sends somebody after a linker.

**Downstream, in `rubix-ai` under a local `[patch]`:** `cargo build` green; UI vitest 8284 passed /
21 failed (all 21 fail on `main` too); `ui/e2e/cases.spec.ts` 3/3 green and repeatable against a live
node; and the entire flow driven by hand with output pasted in
`rubix-ai: docs/testing/insights/cases.md`.

### The two behaviour changes on shipped surfaces

1. **`tags.of` → `tags.find` gate alias.** `tags_of`'s inner gate asked for `mcp:tags.of:call`, a cap
   that exists in **no role bundle**, so the verb was unreachable by every real caller. A shipped test
   encoded that state as correct. The test was changed, deliberately: it pinned a contract no caller
   could satisfy. See `rubix-ai: docs/debugging/insights/gate-alias-was-decorative.md` — and note that
   a missing alias refuses with `Denied`, not `NotFound`, so only a POSITIVE test catches one.
2. **`BootConfig.public_base_url`** — new, `Option<String>`, defaulting to `None`. With `None` the
   existing relative-link behaviour is byte-for-byte unchanged, so **no embedder changes behaviour by
   upgrading**; an embedder that sets it gets absolute links in the mail it sends.

**`invite.email.*` still ships a relative link, and was deliberately left alone.** It has the same
bug, and fixing it here would have bundled an unrelated surface into a PR whose reviewable claim is
the case plane. It is worth its own change.

### FILE-LAYOUT: the nine oversized test files, split

`check-file-size.sh` is a RATCHET, and read per-file it showed this branch adding sixteen violations
to a backlog the script exists to shrink. Two were source files it had pushed over on its own
(`email_target.rs` 389→463, `ladder_test.rs` 278→401) and are fixed in their own commit. The other
nine were new TEST files, 404 to 1286 lines.

All nine are now split by concern, following the repo's OWN two precedents rather than inventing a
third:

* **`crates/host/tests/`** — `agent_suite.rs` already declares 29 files in `tests/agent/` as modules
  of one harness, because Cargo links the whole dependency graph (SurrealDB, Zenoh, wasmtime) into
  every top-level `tests/*.rs` at ~1 GB each. The eight case suites follow it: `case_suite.rs`
  aggregates 30 files under `tests/case/`, each a `*_support.rs` fixture module plus the focused
  files that use it. Eight binaries became one.
* **`role/gateway/tests/`** — `common/` is the established idiom there, and its own header says it
  exists "to stay under the FILE-LAYOUT 400-line limit". `case_token_test.rs` split into
  `case_token_test.rs` (the happy path) + `case_token_denied_test.rs` (the refusals) over a new
  `common/case_request.rs`.

Test count is identical either side: **86** in `case_suite` (15+18+9+9+11+10+8+6) and **5** across
the two gateway files. The largest new file is 307 lines.

*Rejected: one shared `case/support.rs` for all eight suites.* Tempting — `principal()` is
byte-identical in all eight — but everything else is not: `call`, `seed_roster`, `raise_input`,
`case_of` and `seed_insight` each have a different variant per suite (different caps, different
seeds, different signatures). Merging them would have been a behaviour-carrying rewrite of eight
green suites to satisfy a line count. Each suite keeps its own fixtures, moved verbatim.

*Rejected: re-baselining with `--update`.* The script says the list "may only SHRINK", and adding
nine entries to the backlog to make a job green is exactly the move that made it red on master in the
first place.

**Still outstanding, and stated in the PR:** six baseline files grew — `tool_call.rs` +86,
`config.rs` +59, `server.rs` +58, `builder.rs` +24, `lib.rs` +20, `builtin_roles.rs` +8. Growth is
structural in all six: a new verb IS a dispatch arm, a new `BootConfig` field IS a line in
`config.rs`, a new route IS a registration in `server.rs`. Splitting them is a core refactor with a
blast radius far wider than this branch.

## What is not done

- **Not merged, not tagged.** The PR is open for review only. rubix-ai's `feat/case-plane` depends on
  this branch and carries a local `[patch]`; its pin bump waits on a `node-v*` tag cut from this.
- **The `rule.scorecard` N+1 is still there** — one point read of the primary insight per resolved
  case, memoized per insight id. The fix is an `origin_ref` echo written onto the case at open, which
  collapses it to a single scan with zero reads. It is a wave-1 *record* change and wave 1 was
  already committed, so it is named in the module docs rather than done.
- **Storm folding is unbuilt.** `packs/bas` seeds the shape (12 flatlines from one producer, at one
  site, in one sweep) so the fold has something to fold; today they are 12 separate cases and the
  queue shows the burst unfolded. Nothing in the pack changes when the fold lands.
- **Unstarted fast-follows named in the scope:** the `month_hist` seasonal ladder, a verify reactor +
  `case.accept_saving`, the owner report dashboard, and `rule_policy` demotion.
- **`case.request.withdraw` and `case.request.nudge` have no UI caller.** Both are implemented and
  unit-tested; neither is driven end to end, so neither is proven on the wire the way `send`, `view`
  and `reply` now are.
