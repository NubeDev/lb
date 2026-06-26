# `cargo test --workspace` is OOM-killed (exit 137) once multiple nodes boot per test

- Area: bus
- Status: resolved
- First seen: 2026-06-26
- Resolved: 2026-06-26
- Session: ../../sessions/sync/multi-node-sync-session.md
- Regression test: n/a (a test-harness/runner constraint, not a product bug — guarded by a documented run recipe + CI note)

## Symptom

Running the whole suite at once after S3 landed:

```
$ cargo test --workspace
<no per-test output> ... Exit code 137
```

Exit 137 = SIGKILL from the OOM killer. Individual crate suites (`cargo test -p lb-host
--test offline_sync_test`) pass fine; only the *combined* run dies.

## Reproduce

`cargo test --workspace` on a memory-constrained box once the S3 tests exist. Each S3 test
boots **two** `Node`s (edge + hub), and each `Node` opens an embedded **Zenoh peer** (which
itself spins up a transport runtime, link managers, scouting, etc. — not cheap). cargo runs
test binaries in parallel, and within a binary the default harness runs tests in parallel too.
So the peak is roughly `(#test-binaries) × (#tests-per-binary) × 2 peers` live at once →
hundreds of Zenoh peers → OOM.

## Investigation

- Ruled out a leak in the new code: a single binary (even `offline_sync_test`, the heaviest at
  2 nodes × 3 tests) runs green and frees promptly.
- Confirmed the trigger is *concurrency of node booting*, not any one test: the full run died,
  the per-binary runs didn't.
- The pre-existing rule "any test that boots a Node needs the multi-thread Zenoh runtime"
  (zenoh-needs-multi-thread-runtime.md) is about *correctness*; this is about *resource
  ceiling* — a second, related Zenoh cost that only shows up once tests routinely boot 2 nodes.

## Root cause

Embedded Zenoh peers are heavyweight, and S3 doubled the peers-per-test (edge **and** hub). The
default cargo/libtest parallelism multiplies that across binaries and tests, exceeding RAM. It
is a runner-resource constraint, not a logic bug — nothing leaks, there are just too many live
peers at one instant.

## Fix

Bound the concurrency rather than the code. Two equivalent recipes (documented in the session
and the testing notes):

- Per-binary: `cargo test -p <crate> --test <name>` (what this session used to capture green
  output) — caps live peers at one binary's worth.
- Whole suite: `cargo test --workspace -- --test-threads=1` and/or
  `CARGO_BUILD_JOBS` / running heavy crates separately, so peers don't all stack up.

Compile once with `cargo test --workspace --no-run`, then run the binaries in small groups.

## Verification

`cargo test --workspace --no-run` builds all binaries; running them per-binary (or in small
groups) is green across the board — host (spine/messaging/presence/hot_reload/**cross_node**/
**offline_sync**), gateway, and every unit crate. No OOM when concurrency is bounded.

## Prevention

- **Recipe, not retry:** the session doc and `scope/testing` record "run S3 node-booting suites
  per-binary or with `--test-threads=1`" so the next person doesn't rediscover the OOM.
- **CI note:** CI should run the node-booting integration suites with bounded parallelism (a
  job-level `--test-threads` or split jobs), the same way it already pins the multi-thread
  Zenoh flavor. A future `#[lb_test]` harness could also gate how many nodes boot concurrently.
- Related: [zenoh-needs-multi-thread-runtime](zenoh-needs-multi-thread-runtime.md) (correctness)
  and [in-process-peers-share-the-keyspace](in-process-peers-share-the-keyspace.md) (why each
  test needs a unique workspace id).
