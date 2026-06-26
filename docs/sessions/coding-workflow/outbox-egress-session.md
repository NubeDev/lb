# Coding workflow — the outbox egress: real GitHub `Target` + relay backoff/dead-letter (session)

- Date: 2026-06-26
- Scope: ../../scope/inbox-outbox/outbox-scope.md
- Stage: S7 — platform maturity (STAGES.md). Two of the outbox's listed follow-ups, shipped together:
  the **real `Target` adapter** (the in-test target was the only stub) and **backoff + dead-letter**
  (the outbox scope's top open question). The S7 exit gate was already MET.
- Status: done

## Goal

Complete the outbox's **egress edge** and harden its relay:
1. A **real GitHub HTTP `Target`** (`lb-role-github-target`) that delivers outbox effects (open a PR,
   post a comment) to the GitHub REST API — filling the last mock behind the host `Target` seam, the
   egress counterpart to the `lb-role-github-webhook` ingress.
2. **Backoff + dead-letter** in `lb-outbox` + the host relay: a perpetually-failing effect stops
   retrying after `max_attempts` (parked for audit), and a recently-failed effect waits out an
   exponential backoff before the relay retries it — instead of hammering a down target every pass.

## What changed

**Core (`lb-outbox` + host relay) — backoff + dead-letter:**

- `model.rs`: new `EffectStatus::DeadLettered` (terminal); `Effect` gains `max_attempts`
  (default `DEFAULT_MAX_ATTEMPTS = 5`) and `next_attempt_ts` (the backoff gate). A pure
  `backoff(attempts)` (exponential, capped at 64 logical-ts units) + an `Effect::with_max_attempts`
  builder. `Effect::new`'s signature is unchanged (the new fields default), so its call sites are
  untouched.
- `mark.rs`: `mark_failed` is now attempt-aware — it takes `now`, counts the attempt, and either
  **dead-letters** (at `max_attempts`) or leaves it `Failed` with `next_attempt_ts = now +
  backoff(attempts)`. It returns the resulting `EffectStatus` so the relay can tally without a
  re-read. Both marks now share one read-modify-write `update` helper.
- `pending.rs`: `pending` is unchanged (schedulable scan, naturally excludes the new terminal
  status). New `due(store, ws, now)` = schedulable AND past the backoff gate (what the relay
  attempts), and `dead_lettered(store, ws)` (the parked poison messages, for audit/replay).
- `workflow/relay.rs`: `relay_outbox` takes `now`, scans `due` (not `pending`), and its `RelayPass`
  gains a `dead_lettered` tally. A failure routes through `mark_failed`, which decides backoff vs
  dead-letter in one place.

**Role crate (`lb-role-github-target`) — the real adapter:**

- `request.rs`: pure `Effect → (path, json-body)` mapping for `create_pr` and `comment`. An unknown
  action / bad payload / wrong target is a **permanent** `MapError` (distinct from a transient
  transport failure). Unit-tested in-file.
- `client.rs`: `GithubTarget` implements the host `Target` over `reqwest`. Idempotency: a `create_pr`
  that returns `422 "already exists"` is treated as **delivered** (GitHub is the dedup oracle — a
  re-delivery never opens a second PR); the `idempotency_key` also rides as a header. A `5xx`/network
  error is transient (`Err` → retry with backoff); the token is private and never logged.
- `lib.rs`: the wiring + the contract doc.

Workspace: added `role/github-target` to `members` + the path dep. No SDK/WIT or capability-grammar
change. `reqwest` stays in the role crate, never core (roles depend on host, never the reverse).

## Decisions & alternatives

- **Backoff in the relay, not in `pending`.** `pending` keeps its `(store, ws)` signature (the audit
  view); the relay calls the new `due(store, ws, now)` for the backoff-gated subset. This avoided
  rippling `pending`'s arity through a dozen call sites while still gating retries.
- **`Effect::new` signature preserved.** New fields default; a builder (`with_max_attempts`) sets the
  cap when needed. Rejected: adding two params to `new` (would have churned every producer).
- **422-as-success is the real idempotency for `create_pr`.** GitHub rejects a duplicate PR for the
  same head with `422` — treating that as delivered is what makes at-least-once safe without a
  search-before-create round-trip. The `idempotency-key` header is belt-and-suspenders for a
  dedup-aware proxy. (A general search-before-create for non-422 cases is a noted follow-up.)
- **Permanent vs transient mapping faults.** A bad payload/unknown action can never succeed, so it
  fails the effect — but the dead-letter cap (now in place) stops the futile retries rather than
  looping forever. This is exactly why the two halves shipped together.
- **`now` threaded through the relay, no wall-clock.** Backoff is computed against an injected `now`
  (testing §3), so the backoff/dead-letter tests are deterministic.

## Tests

All green; the only externals mocked are the GitHub origin (a fake on `127.0.0.1:0`) and, in the host
workflow tests, the in-test target — store/bus/relay are real.

- `lb-outbox` (`outbox_test.rs`): +2 new — **backoff** (a failed effect is owed but not `due` until
  its gate elapses) and **dead-letter** (exhausting `max_attempts` parks it, off the schedulable set,
  visible via `dead_lettered`). The retry test updated to the new `mark_failed(.., now)` signature.
- `lb-host` workflow tests (regression): the three `relay_outbox` callers updated to pass `now` and
  advance it past the backoff between a fail and a retry. Still green (8: full gate, isolation,
  offline at-least-once + dedup).
- `lb-role-github-target` (new, 9): 5 unit (the action mapping + permanent-error branches) + 4
  integration over a real socket — **happy** delivery (201), **idempotency** (422 → delivered, no
  second PR), **dead-letter** (always-5xx → parked at the cap, through the real adapter), and a
  **transport failure** (unreachable origin → schedulable, then delivered once it recovers).

Green output:

```
$ cargo test -p lb-outbox --test outbox_test
running 6 tests
test enqueue_writes_the_change_and_the_effect_atomically ... ok
test re_enqueuing_the_same_effect_id_is_idempotent ... ok
test a_failed_delivery_stays_schedulable_and_redelivers ... ok
test a_failed_effect_waits_out_its_backoff_before_it_is_due ... ok
test an_effect_dead_letters_after_exhausting_max_attempts ... ok
test an_effect_is_invisible_across_the_workspace_wall ... ok
test result: ok. 6 passed; 0 failed

$ cargo test -p lb-host --test workflow_test --test workflow_isolation_test --test workflow_offline_test
workflow_isolation_test:  ok. 2 passed
workflow_offline_test:    ok. 3 passed
workflow_test:            ok. 3 passed

$ cargo test -p lb-role-github-target
running 5 tests (unittests src/lib.rs)
test request::tests::create_pr_maps_to_the_pulls_endpoint ... ok
test request::tests::comment_maps_to_the_comments_endpoint ... ok
test request::tests::a_foreign_target_is_not_ours ... ok
test request::tests::an_unknown_action_is_a_permanent_error ... ok
test request::tests::a_malformed_payload_is_a_permanent_error ... ok
test result: ok. 5 passed; 0 failed
running 4 tests (tests/github_target_test.rs)
test a_create_pr_effect_delivers_to_github_over_http ... ok
test an_already_exists_422_is_idempotent_success ... ok
test a_persistently_failing_target_dead_letters_after_the_cap ... ok
test a_transport_failure_leaves_the_effect_schedulable ... ok
test result: ok. 4 passed; 0 failed

$ cargo build --workspace                       # green
$ cargo fmt / clippy (lb-outbox, lb-host, lb-role-github-target)   # clean
$ git add -A && cd rust && bash scripts/check-file-size.sh
FILE-LAYOUT: all source files within 400 lines (320 checked)
```

So **+2 outbox + 9 github-target = 11 new** green; 8 workflow regression updated + green.

## Debugging

None — nothing broke. The signature changes to `relay_outbox` / `mark_failed` were caught at compile
time and the call sites updated in the same change.

## Public / scope updates

- Promote to `public/inbox-outbox/inbox-outbox.md` (the outbox section) + `public/SCOPE.md`: the real
  GitHub `Target` + relay backoff/dead-letter.
- Resolve the outbox scope's **"Backoff + dead-letter"** open question (now answered) and the **real
  `Target` adapter** follow-up (GitHub HTTP shipped). Remaining opens refreshed: email/sync-publish
  adapters, the multi-relay atomic claim, FIFO-per-target ordering, the LIVE-query relay reactor, and
  a resolution reactor that auto-starts the job on approval.

## Dead ends / surprises

- **The two halves needed each other.** A real adapter makes permanent failures (bad payload, unknown
  action) reachable; without the dead-letter cap, those would retry forever. Shipping backoff +
  dead-letter alongside the adapter is what makes the egress edge actually safe to run on a loop.
- **GitHub's 422 is a feature.** What looks like an error response is the natural idempotency oracle
  for PR creation — leaning on it avoids a search-before-create round-trip and keeps the adapter thin.

## Follow-ups

- **Producer payload enrichment.** `start_job.rs` currently emits a `create_pr` payload of just
  `{scope_doc}`; the real adapter expects `{repo, head, base, title, body}`. Enriching the producer
  (or adding a payload-shaping step) is the next link to a truly live PR. (Open.)
- **Email / sync-publish `Target`s** behind the same trait; **search-before-create** for non-422 dedup.
- The **multi-relay atomic claim**, **FIFO-per-target ordering**, the **LIVE-query relay reactor**, and
  a **resolution reactor** that auto-starts the job on approval (cross-linked from the webhook session).
- STATUS.md: add the slice row.
