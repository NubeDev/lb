# `a_live_node_that_drops…` fails in-suite but passes alone — pre-existing, not a regression

- Area: bus
- Status: open (pre-existing flake; triaged, not fixed)
- First seen: 2026-07-20
- Session: ../../sessions/mcp/routed-dispatch-sidecar-bridge-session.md
- Regression test: n/a — this is a harness/timing constraint in an existing test, not a product bug

## Symptom

`routed_ambiguity_test::a_live_node_that_drops_becomes_unreachable_promptly_and_runs_nothing`
fails when the file's 12 tests run together:

```
test a_live_node_that_drops_becomes_unreachable_promptly_and_runs_nothing has been running for over 60 seconds
test a_live_node_that_drops_becomes_unreachable_promptly_and_runs_nothing ... FAILED
test result: FAILED. 11 passed; 1 failed; finished in 119.28s
```

The other 11 pass. Run alone (`--test-threads=1`, or filtered to just this test) it passes
every time, in ~31s.

## Why this entry exists

It surfaced during the routed-dispatch-sidecar-bridge session, which changed the dispatch path
this suite exercises — so it *looked* like a regression from that work. It is not.

## Proof it is pre-existing

Ran the same suite in a detached worktree at **`36ae877d`** (the session's base commit, none of
the session's changes present):

```
test a_live_node_that_drops_becomes_unreachable_promptly_and_runs_nothing ... FAILED
test result: FAILED. 11 passed; 1 failed; finished in 176.68s
```

Byte-identical failure, same 11-pass/1-fail split, on unmodified master. The session's change is
**not implicated**.

Method note: the first attempt at this comparison used `git stash`, and the background test run
was killed when the stash popped — producing empty output that could have been misread as
"no failure on clean tree." A detached worktree is the reliable way to run a clean-tree
comparison while keeping your working tree intact.

## Cause (probable)

The same family as [`routed-call-races-mesh-discovery.md`](routed-call-races-mesh-discovery.md)
and [`cargo-test-workspace-ooms-with-many-peers.md`](cargo-test-workspace-ooms-with-many-peers.md):
this test asserts a dropped node becomes `NodeUnreachable` **promptly** (a bounded wait, deliberately
tighter than zenoh's ~10s query default — that tightness is the point of the test). Each of the 12
tests in the file boots 2–3 Zenoh peers, so running them concurrently puts ~30 peers on the box;
under that load, queryable-retraction propagation routinely exceeds the bound. A timing assertion
calibrated for an unloaded box, measured on a loaded one.

Consistent with the timings: 31s alone → 119s in-suite → 177s in the worktree run.

## Workaround

Run it isolated when it matters:

```
cargo test -p lb-host --test routed_ambiguity_test a_live_node_that_drops -- --test-threads=1
```

## Not done here

No fix attempted — the change would be to the drop test's timing bound or the suite's peer
concurrency, both owned by routed-node-dispatch/fleet-presence, and neither is this session's
scope. Flagged so the next session in this area does not re-derive the triage. If it starts
failing in CI rather than only locally, it needs a real fix rather than this note.
