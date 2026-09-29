# Session — case self-clear

- **Status:** done (ready for review)
- **Started:** 2026-09-29
- **Branch:** `feat/case-self-cleared`
- **Scope:** [`docs/scope/insights/case-self-clear-scope.md`](../../scope/insights/case-self-clear-scope.md)
- **Downstream:** `NubeIO/rubix-ai` bumps its `lb-node` pin once this is tagged; ESR opts in with one
  `policy.sla.set` call.

## The ask

On the ESR node the Insights page's "Resolved" tile read 3 while an insights CSV export listed ~395
resolved rows: rules had resolved their insights (the meters recovered) but every paired case stayed
in `to_action`, because the case owns triage and nothing closes a case but a person. 393 of 1,182 open
cases had every insight resolved. Asked for the fix to live in lb, as an opt-in, not as a script on
the node.

## What was built

- `lb_cases::ServicePolicy.auto_close_self_cleared: bool` (`#[serde(default)]` ⇒ `false`; existing
  rows decode unchanged). Every in-tree `ServicePolicy { .. }` literal sets it.
- `host/src/case/self_clear.rs::close_self_cleared_cases(node, ws, now)` + `SELF_CLEAR_ACTOR =
  "system:self-clear"`, re-exported from `lb_host`.
- Called from `case/reactor.rs` on the existing 60 s case tick, after reconcile and the facet backfill.
- Decision, per open case under an opted-in policy: every member insight resolved (primary as the
  fallback when there are no member rows) AND untouched (`to_action`, no assignee / waiting-on /
  snooze, `reopened_count == 0`, every history event from a `system:` actor) ⇒
  `lb_cases::workflow(Resolved, SelfCleared, system:self-clear)`.

## Tests — real node, `mem://`, no mocks

`host/tests/case/reactor_self_clear.rs` (in `case_suite`):

```
cargo test -p lb-host --test case_suite reactor_self_clear
running 6 tests
......
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 119 filtered out
```

Green-while-broken check: with the history guard replaced by `Ok(true)` (the pass ignores who worked
the case) the suite goes red exactly where it should —

```
reactor_self_clear::a_worked_case_stays_for_a_person --- FAILED
test result: FAILED. 5 passed; 1 failed
```

— then the guard was restored. Regression, whole suites:

```
cargo test -p lb-host --test case_suite   → ok. 125 passed; 0 failed
cargo test -p lb-cases                    → ok. 34 + 17 + 7 passed; 0 failed
cargo fmt --all --check                   → clean
rust/scripts/check-file-size.sh           → 31 findings before and after (all pre-existing on master;
                                            no file this branch touches is new or grown past baseline)
```

## End to end — a real rubix-ai node on this branch

rubix-ai `main` built against this checkout through its local `[patch]` (`make cloud` with a scratch
store; the binary carries `system:self-clear`), driven only through the HTTP gateway as a signed-in
user, the close done by the node's own 60 s tick:

```
PASS policy opted in / policy lists the flag
PASS raise A, B, C — each has a case, each case carries policy_id "default"
PASS comment on B's case
PASS resolve A's insight / resolve B's insight
PASS pre-tick: all three cases open
  closed after 50 s
PASS A (untouched, cleared) resolved / resolution self_cleared / resolved_by system:self-clear
PASS B (commented) stays open
PASS C (still firing) stays open
PASS A history has the resolved event by system:self-clear
PASS A fires again -> a NEW open case / old case keeps its self-cleared history
E2E PASS
```

Then the product surface, in a browser against the same node: an insight D raised, resolved, and
self-cleared by the tick 5 s later. The Insights page read **Resolved 1**, D's row **Resolved** with a
*Reopen* action; the page's CSV export listed D as `resolved`. B (commented) is `resolved` in the
export and `To be actioned` on the page — by design, a worked case stays for a person.

## Notes for the next change

- The resolve verb cannot tell a rule from a person; the scope explains why the feature does not need
  to. If it ever must, the seam is `rules/src/verbs/insight.rs` `close`, which could pass an origin.
- `docs/STATUS.md` and the public insights page should mention the flag when this ships to a customer.
