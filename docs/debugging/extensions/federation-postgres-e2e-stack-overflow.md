# federation_end_to_end_postgres aborts with a stack overflow

**Date found:** 2026-07-12 (during the `pack-toolchain-publish` session's full-workspace run)
**Area:** datasources / federation (NOT extensions-packaging — filed here because this session found it)
**Status:** open — pre-existing on master-equivalent code; not chased per the "don't chase
someone else's regression" rule.

## Symptom

```
thread 'federation_end_to_end_postgres' (…) has overflowed its stack
fatal runtime error: stack overflow, aborting
error: test failed … (signal: 6, SIGABRT)
```

Deterministic across two runs on this box (`cargo test -p lb-host --test federation_test
federation_end_to_end_postgres`). The test spawns a real docker postgres and builds the
federation sidecar in-test; the overflow happens after that, in the test thread.

## Why it is not the pack-toolchain change

The `pack-toolchain-publish` diff touches only `crates/devkit` internals (a struct rename +
`cfg(feature)` gates whose compiled behavior with `devkit-full` on — the workspace default —
is identical) and Cargo metadata (`publish` flags, feature wiring). `git diff master...HEAD
-- crates/host crates/federation role` shows **no source change** in any code this test
executes. A runtime stack overflow in the postgres/polars pushdown path cannot be produced by
packaging metadata. (A clean-master rebuild to reproduce was aborted: a second full debug
target dir filled the disk — this box can't hold two workspace builds.)

Related known context: `federation_count_star_columnless_aggregate` in the same file is
already `#[ignore]`d for a postgres-pushdown bug
(`docs/debugging/datasources/count-star-aggregate-schema-mismatch.md`); the broad
pre-existing failing set (persona catalog/coding, reminder, devkit build/e2e, proof_panel) is
recorded in the debugging history. This entry adds `federation_end_to_end_postgres` to the
watch list. Note it may previously have been silently **skipped** on boxes without docker —
docker being available here may be why it only surfaced now.

## Next step (for whoever owns federation)

Run it under a debugger / `RUST_MIN_STACK` bump to find the recursion (sqlparser/polars plan
recursion is the usual suspect), on clean master first.
