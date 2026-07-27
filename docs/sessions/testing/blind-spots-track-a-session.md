# Testing blind spots — track A: the upgrade-path fixture + boot-wiring assertions (session)

- Date: 2026-07-27
- Issue: [#108](https://github.com/NubeDev/lb/issues/108) — "Three bugs shipped past a green suite"
- Scope: `../../scope/testing/testing-scope.md` §2 category 6 (prior-state / upgrade tests) and
  §3.2 row 4 (the boot wiring itself)
- Stage: S8 (data plane), standing test-hardening work
- Status: **done** (track A of the issue's checklist; other tracks are separate sessions)

## Goal

Close two of the four blind spots §3.2 names, with tests that FAIL when the thing they guard is
removed:

1. an **upgrade-path fixture** so "boot against state an older build left behind" is one line rather
   than a hand-rolled JSON literal per test (bug #2 needed three), plus at least one real
   prior-state test using it;
2. **boot-wiring assertions** — until now, deleting a `spawn_*_reactors` line from
   `node/src/reactors.rs` broke no test at all.

## What changed

| File | Responsibility |
|---|---|
| `rust/crates/host/tests/support/prior_state.rs` (new) | The prior-state factories: `PriorRetention` (policy rows an older build left on disc — per-network rows, tuned rows, and a pre-`max_samples` row SHAPE) and `PriorSeries` (committed history, staged + drained through the real ingest path). Shared via the standard `#[path = "support/…"] mod …;` pattern, because integration tests are separate crates. |
| `rust/crates/host/tests/series_prior_state_test.rs` (new, 2 tests) | The upgrade tests: bug #2's precedence shape, and the pre-cap row shape. |
| `rust/node/tests/boot_wiring_test.rs` (new, 2 tests) | The boot wiring, asserted through `lb_node::reactors::spawn` — never an individual spawner. |
| `rust/node/Cargo.toml` | `lb-ingest` as a **dev**-dependency (the boot-wiring test seeds and counts real series rows; the binary itself still reaches the series plane through `lb-host`). |
| `rust/crates/ingest/src/retention.rs` | **Bug fix** found by the new prior-state test — see §Debugging. |

### The prior-state tests

- `the_new_global_cap_governs_only_after_the_previous_builds_per_network_row_is_removed` —
  reproduces bug #2's shape inside lb: seed the older build's `modbus.plant-a.` row and its history,
  write the newer global `modbus.` cap through the real capability-gated verb (it SUCCEEDS), assert
  a real GC pass evicts **nothing** because the stale longer prefix still owns the series, then run
  the convergence (`series.retention.delete`) and assert the new default now governs — 60 → 10 rows,
  `capped_raw: 50`. It is not a tautology in either direction: phase 1 fails if longest-prefix-wins
  is ever weakened, phase 2 fails if the migration reports success without deleting the row (bug #4's
  shape).
- `a_policy_row_written_before_the_cap_existed_reads_back_unbounded_and_upgrades_in_place` — the
  stored-SHAPE axis. It failed on first run against a real bug (§Debugging).

### The boot-wiring tests

Both call `lb_node::reactors::spawn(&node, ws, &OutboxProviders::default())` and assert the
reactor's property with **nobody calling its verb** — staged samples commit (ingest drain), and a
series over its `max_samples` cap shrinks to the bound (retention GC). The properties are lifted from
`crates/host/tests/series_cap_reactor_test.rs`, where they are proven against a directly-spawned
reactor; the only new thing here is that the spawn comes from the real wiring function.

Determinism: no fixed sleeps — poll-with-timeout (~20 s ceiling) against the reactors' real cadences.
Both reactors' `tokio::time::interval` fires its first tick immediately, so the 300 s retention
period is not a problem for a test; the count-cap axis is used (`raw_for_ms: 0`) so no assertion
depends on a wall clock.

## Decisions & alternatives

**1. The boot-wiring test calls `reactors::spawn`, not the spawners, and not `boot_full`.** Listing
the spawners in a test would reproduce exactly the bug the test exists to kill — the list stays green
while `reactors.rs` loses an entry. *Rejected:* `boot_full` (as `node/tests/relay_boot_test.rs` uses
for the relay reactor): it is the fuller path but drags gateway/port/config posture into a test about
two data-plane reactors, and `reactors::spawn` is the one function whose deletion of a line is the
failure being guarded. `OutboxProviders::default()` — the unconfigured embedder, relay falling back
to its logging no-ops — makes the full `spawn` cheap to call.

**2. The fixture lives in `tests/support/`, not in a shared dev-dep crate.** Integration tests are
separate crates, so `#[path]` module inclusion is the standard (and dependency-free) way to share a
factory; the repo already uses `#[path]` for `agent_suite.rs`. *Rejected:* a `lb-test-support` crate
— a whole published-shaped crate for two builders, and it would tempt a `common`/`utils` dumping
ground (FILE-LAYOUT).

**3. The pre-cap row is seeded with a raw `UPSERT`, not by mutating a `Policy`.** Today's struct
cannot emit a row without `max_samples`, and the statement used is byte-for-byte the one `set_policy`
issues, so what lands is exactly what the older build left. That is a **seed, not a mock** (testing
§0): it feeds the real read path rather than replacing it. *Rejected:* keeping an old copy of the
struct around to serialize with — a second source of truth for a shape, which is the fake this repo
bans.

## Tests

```
cargo test -p lb-host --test series_prior_state_test -p lb-node --test boot_wiring_test --no-fail-fast

running 2 tests
test the_new_global_cap_governs_only_after_the_previous_builds_per_network_row_is_removed ... ok
test a_policy_row_written_before_the_cap_existed_reads_back_unbounded_and_upgrades_in_place ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.25s

running 2 tests
test boot_spawns_the_ingest_drain_so_staged_samples_commit_with_nobody_draining ... ok
test boot_spawns_the_retention_gc_so_a_capped_series_shrinks_with_nobody_calling_the_verb ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.30s
```

Unaffected-neighbour check (the `Policy` deserialization change): `lb-ingest`
`series_retention_test` + `series_sample_cap_test` (3 + 7) and `lb-host` `series_cap_reactor_test` +
`series_lifecycle_test` (4 + 6) all green. `cargo build --workspace` clean;
`bash rust/scripts/check-file-size.sh` → `OK — 2408 files checked, 114 grandfathered` (the backlog is
unchanged; every new file is well under 400 lines).

### Revert-check (restored from a `cp` snapshot, never `git checkout --`)

Commenting out one line in `node/src/reactors.rs` at a time:

```
spawn_ingest_reactors commented out:
test boot_spawns_the_retention_gc_so_a_capped_series_shrinks_with_nobody_calling_the_verb ... ok
test boot_spawns_the_ingest_drain_so_staged_samples_commit_with_nobody_draining ... FAILED
  panicked at node/tests/boot_wiring_test.rs:92:5:
  boot must spawn the ingest drain: without it staged samples commit only when some caller pays
  for the whole backlog inside its own request, which is the bug the reactor exists to fix
test result: FAILED. 1 passed; 1 failed

spawn_retention_reactors commented out:
test boot_spawns_the_ingest_drain_so_staged_samples_commit_with_nobody_draining ... ok
test boot_spawns_the_retention_gc_so_a_capped_series_shrinks_with_nobody_calling_the_verb ... FAILED
  panicked at node/tests/boot_wiring_test.rs:136:5:
  boot must spawn the retention GC: without it a correctly-configured cap is decorative and the
  series grows until the disc is full
test result: FAILED. 1 passed; 1 failed
```

Each sabotage fails **only** its own reactor's test, so the two assertions are independent. The file
was restored from a `cp` copy taken before the first edit and verified by md5 — the process note in
`series-normalize-session.md` (a `git checkout --` restore that destroyed uncommitted work) was
followed.

## Debugging

- [`debugging/ingest/pre-cap-policy-row-aborts-the-retention-pass.md`](../../debugging/ingest/pre-cap-policy-row-aborts-the-retention-pass.md)
  — **fixed**, found by the second prior-state test on its first run. A policy row written before
  `max_samples` existed comes back from the explicit projection as `NONE` (a present null, which
  `#[serde(default)]` does not cover), so `list_policies` — and therefore every `run_gc` pass —
  fails for the whole workspace on an upgraded node. Fixed with a `none_as_default` deserializer on
  `max_samples` and `tiers`. This is the fixture paying for itself in its first hour: the bug is
  unreachable from a suite whose rows are all written by today's struct.

## Follow-ups (not this session)

- The other #108 tracks: multi-batch drain/paging coverage and axis assertions for width-keyed
  behaviour (concurrent sessions).
- The same `NONE`-vs-absent audit is worth running over the other explicitly-projected stored structs
  (dashboards, flows, packs) — this session only fixed the one the retention fixture reached.
- No `doc-site/content/public/` promotion: nothing user-facing shipped, and the `series.retention.*`
  contract is unchanged.
