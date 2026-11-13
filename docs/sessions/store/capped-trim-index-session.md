# Store — capped ring trim scans the whole table (session)

- Date: 2026-11-13
- Scope: ../../scope/observability/telemetry-console-scope.md (the capped ring and its trim cadence)
- Stage: S7 — platform maturity (STAGES.md); a performance fix to a shipped S10 primitive
- Status: done

## Goal

A busy embedded node pegged 4–7 cores with nobody using it. Find the cost, fix it at the shared
primitive, and leave a regression test that fails on the old code.

## What changed

- `rust/crates/store/src/capped_index.rs` (new): `ensure_capped_index` defines
  `capped_key_seq ON TABLE <table> FIELDS cap_key, seq` with `IF NOT EXISTS`, once per
  `(ws, table)` per store, through one `OnceCell` each, so concurrent first writers wait for one
  build. Best effort: a failed build logs WARN `capped ring index not ensured` and the insert goes
  ahead. Refuses a table name that is not a bare identifier (it is interpolated into DDL).
- `rust/crates/store/src/open.rs`: `Store` carries the build slots (`capped_indexes`), so they die
  with the store.
- `rust/crates/store/src/capped.rs`: calls `ensure_capped_index` before the transaction. The trim is
  a cutoff, `$cut[0] != NONE AND seq <= $cut[0]` with `$cut` = the first row past the newest `cap`,
  instead of `seq NOT IN $keep`. Stale comments fixed; amortized trim added to "Considered and
  rejected".
- `rust/crates/store/src/write_batch.rs`: `is_bare_ident` is `pub(crate)` so the index reuses it.
- `rust/crates/telemetry/src/layer.rs`: the trim-cadence note now says why strict trimming is cheap.

Callers are unchanged: the telemetry layer and seed, insight occurrences, entity versions.

## Decisions & alternatives

- **Index, not amortized trimming.** The telemetry-console scope deferred amortized trim (every *m*
  inserts) "until a benchmark shows the strict trim costs". This was that benchmark. Amortizing cuts
  the cost by *m* but every trim still scans the ring, and gives up "never exceeds cap". The index
  makes each trim proportional to one key's rows and keeps the invariant. Rejected amortizing.
- **Cutoff, not keep-list.** `seq` is unique per row (it is the record id), so "at or older than the
  (cap+1)th newest" deletes exactly the rows `NOT IN $keep` did, without comparing every row to a
  1000-item array. The `$cut[0] != NONE` guard keeps the under-cap case identical too (otherwise a
  row with no `seq` would match `NONE <= NONE`).
- **Built once, single-flight, on the `Store`.** The job index re-runs `DEFINE INDEX IF NOT EXISTS`
  before every create. A capped table takes a write per telemetry event, and SurrealDB starts a fresh
  build for every concurrent `DEFINE INDEX`, so N racing first writers would scan the ring N times at
  once right after an upgrade. A process-wide static keyed on the store's address was rejected: a
  dropped store's address can be reused by the next one.
- **Best effort, not fatal.** The trim is correct without the index. Returning the DDL error would
  drop a telemetry row silently (the layer is fire-and-forget) and fail an insight raise after its
  parent counter had moved.
- **Table name validated with the existing `is_bare_ident`**, not escaped and not a third copy of
  the check.

## Tests

Mandatory categories: **workspace isolation** applies and is covered
(`trim_index_is_ensured_in_every_workspace`). **Capability-deny** does not apply: `capped_insert` is
a store verb that runs after `caps::check` and is not an authorization point (`capped.rs` module
note); the callers' own deny tests (`telemetry_test`, `versions_authz_test`) ran unchanged.
Offline/sync and hot-reload do not apply (a capped ring is per-node, not synced).

New file `rust/crates/store/tests/capped_index_test.rs`: `trim_reads_one_key_through_the_index`
(plan is an `IndexScan` on `capped_key_seq`, never a `TableScan`), `non_identifier_table_is_refused`,
`trim_index_is_ensured_in_every_workspace`,
`existing_unindexed_ring_is_indexed_and_trimmed_on_first_insert`,
`concurrent_first_inserts_on_an_unindexed_ring_all_land`,
`insert_lands_and_trims_when_the_index_cannot_be_built`. Added to `capped_test.rs`:
`lowering_the_cap_trims_to_the_new_cap_in_one_insert`. The index tests got their own file so
`capped_test.rs` stays under the 400-line limit (`check-file-size.sh` flags nothing under
`crates/store/`; the script already fails on 41 files elsewhere on the base tag).

Revert checks: the index and identifier tests fail on the old code (source stashed); the workspace
test fails if the remembered key drops the workspace; the failed-build test fails if the DDL error
is returned instead of logged.

```text
$ cargo fmt --all --check            # clean
$ cargo test -p lb-store --test capped_test --test capped_index_test
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.25s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s

$ cargo test -p lb-store -p lb-telemetry -p lb-insights --no-fail-fast
52 test binaries, every one "test result: ok", 210 passed, 0 failed

$ cargo test -p lb-host --no-fail-fast --test versions_test --test versions_authz_test \
    --test telemetry_test --test insights_test --test insight_triage_test \
    --test insight_evidence_test --test insight_analysis_test --test insight_case_lens_test
test result: ok. 16 passed; 0 failed   (insight_analysis_test)
test result: ok. 2 passed; 0 failed    (insight_case_lens_test)
test result: ok. 10 passed; 0 failed   (insight_evidence_test)
test result: ok. 17 passed; 0 failed   (insight_triage_test)
test result: ok. 22 passed; 0 failed   (insights_test)
test result: ok. 7 passed; 0 failed    (telemetry_test)
test result: ok. 8 passed; 0 failed    (versions_authz_test)
test result: ok. 9 passed; 0 failed    (versions_test)
```

The full `cargo test --workspace` was not run locally (CI shards it; it needs well over 100 GB of
`target/`). The change is internal to `lb-store` (no public signature changed), and every crate that
calls `capped_insert` was tested above.

Measured once, locally, in a debug build against a `mem://` ring of 20 keys × 1000 rows (timing
harness not committed): 621 ms per insert before, 6.8 ms after; ~384 ms one-off for the first insert
after upgrade, which builds the index over the existing rows.

## Debugging

[`debugging/store/capped-insert-trim-scans-whole-ring.md`](../../debugging/store/capped-insert-trim-scans-whole-ring.md),
regression tests above.

## Public / scope updates

- `doc-site/content/public/upgrading/upgrading.md`: new note, "Capped rings get a trim index on
  first write".
- `scope/observability/telemetry-console-scope.md`: the resolved "Trim cadence" question records this
  benchmark and the decision; the indexes risk notes that only `(cap_key, seq)` ships.
- `STATUS.md`: the telemetry-console row's deferred "amortized trim cadence" now points here.

## Skill docs

n/a: no agent- or API-drivable surface changed. No verb, payload or route moved.

## Dead ends / surprises

- The node's stack samples could not name the caller: SurrealDB runs queries on its own router task.
  Pausing each extension sidecar in turn (no CPU change) is what showed the cost was per call, not
  one caller.
- A failing `flow-run retention sweep` every 2.5 min looked like the cause and was not; CPU did not
  dip after it failed. It is a separate open issue (below).
- Deleting a scratch test file mid-build silently failed a test run whose output filter hid errors.

## Follow-ups

- `telemetry.query`/`tail`/`trace` still scan the ring (no `ws`/`trace_id`/`level`/`source` index).
- `insights/src/occurrences.rs` filters on `insight_id`; it could filter on `cap_key` to use the new
  index.
- The `flow-run retention sweep` fails every 2.5 min with "cancelled transaction" on a workspace with
  no `flow_run`/`flow_step_output` tables. Not investigated to a cause yet.
- From review, optional and not done: a time limit on the index build (a DDL that never returns
  would hang every capped write to that table; none is known), and rate-limiting the WARN while a
  build keeps failing (today one line per insert). Only the trim read's plan is pinned by a test;
  the DELETE runs on SurrealDB's older executor, which `EXPLAIN` does not cover.
- Council: three independent reviews (trim semantics, upgrade/ops risk, repo rules and tests), each
  re-run after the fixes; all three returned "ship".
- STATUS.md updated: yes (telemetry-console row).
