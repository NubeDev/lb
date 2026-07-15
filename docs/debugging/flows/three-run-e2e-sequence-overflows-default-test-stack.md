# A three-run e2e test sequence overflows the default test-thread stack (debug builds)

- Area: flows (test infrastructure; the engine is unaffected)
- Status: resolved
- First seen: 2026-07-15 (writing the `lastError` read-back regression for the readback-hardening
  session)
- Resolved: 2026-07-15
- Session: ../../sessions/flows/flows-readback-hardening-session.md
- Regression test: n/a (the resolution reshaped the test itself; the two halves are
  `failed_firing_marks_last_error_and_keeps_the_good_value` and
  `next_ok_firing_clears_last_error` in `flows_scan_paging_test.rs`)

## Symptom

A new e2e test driving one flow through **three** sequential runs (ok → node.update → failing run →
node.update → ok) aborted with `thread '…' has overflowed its stack` / SIGABRT — deterministically,
even alone with `--test-threads=1`. Every other test in the binary passed.

## Investigation

- `RUST_MIN_STACK=3145728` (3 MiB) makes it pass in 2.5 s — so bounded-but-deep frames, not
  runaway recursion. The engine's genuine async-recursion seams are already `Box::pin`ed
  (`coordinator.rs`, `run.rs`, `execute_node/*` — see
  `async-run-not-send-recursion.md`).
- Bisected in a clean worktree (the main tree was mid-edit by a concurrent session): the new
  `lastError` write on the Err path was **exonerated** — removing it entirely still overflowed.
- Stage probes all pass individually at the default stack: ok-run → update → rerun (probe A);
  a first-run failure (probe B); ok → update → failing run (probe C); probe C + the node_state
  read (probe D). Only the full three-run sequence overflows.

## Root cause

Debug-profile poll-chain depth. Under `#[tokio::test(flavor = "multi_thread")]` the test body's
sequential verb calls are polled inside `block_on` on the 2 MiB libtest thread, and each
store-backed verb descends through large unoptimized SurrealDB/serde poll frames. The mainline
flows paths sit just under the 2 MiB edge (the repo already carries two admitted pre-existing
federation e2e overflows of the same class); the three-run sequence lines up a poll chain just past
it. No single stage is the culprit — the length of the longest inline poll chain is.

## Fix

Split the one ok→err→ok test into two half-length tests covering the same contract: (A) a failed
firing marks `lastError` and keeps the last good value; (B) a flow whose FIRST run fails reads back
the error, and the next Ok clears it. Both pass at the default stack and default parallelism.
Rejected alternative: raising `RUST_MIN_STACK` for the suite — it hides the class instead of
staying under it, and every future test would inherit the dependency on the env var.

## Lesson / prevention

In debug builds, an e2e test's stack headroom is consumed by the **longest single poll chain**, and
long sequential flow scripts in one test body walk right up to the 2 MiB default. Prefer two short
scenario tests over one long saga — the assertions are the same, the depth halves, and a failure
localises better anyway. If a genuinely long sequence is ever unavoidable, that is the moment to
discuss a suite-level stack policy, not a per-test env hack.
