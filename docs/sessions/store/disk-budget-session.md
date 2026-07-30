# Session — a node disk budget (bound bytes, not just rows)

Scope: [`scope/store/disk-budget-scope.md`](../../scope/store/disk-budget-scope.md).
Issue [#122](https://github.com/NubeDev/lb/issues/122). All three slices shipped in one session.

## What shipped

**Slice 1 — the budget is a number the operator picks.**
`BootConfig::store_budget_bytes: Option<u64>`, parsed from `LB_STORE_MAX_BYTES` in
`node/src/config.rs::from_env` on the `max_extension_upload_bytes_from_env` pattern: unset, empty
or unparseable ⇒ `None`, with a warn to stderr, never a panic in boot config. The mark arithmetic
lives in one pure file (`host/src/store_admin/marks.rs`): `budget_marks(Option<u64>) -> BudgetMarks`
with `SOFT_MARK_PCT = 80` / `HARD_MARK_PCT = 95` (decision 1). `None` ⇒ `threshold_bytes ==
LOG_ADVISORY_BYTES` and **no marks at all** — today's behaviour byte-for-byte. `StoreStatusReport`
gained `budget_bytes`, `headroom_bytes` and `free_disk_bytes`, and the budget now reaches the
`store.status` verb through a `Node::install_store_budget` / `Node::store_budget()` seam (the same
install-at-boot posture as `gateway_url`, the signing `key` and the response cache) — so an operator
reads the allowance and the headroom, not just the size. `store/src/status.rs::log_stats` now adds
the `manifest` bytes to the sum (decision 4).

**Slice 2 — the reactor acts** (approved by the measurement below).
`store_admin/budget.rs` holds the driver: `BudgetDriver::decide(log_bytes, now) -> BudgetAction`,
pure, no store and no clock of its own. Past the soft mark it enqueues **one** `store.compact` job
in `BootConfig::workspace` with `requested_by = "system:store-budget"` (decision 8 — never a
fan-out; the pass is node-global and each one quiesces every write on the node). Three guards:
a budget must exist, `AUTO_COMPACT_MIN_INTERVAL` (one hour) between automatic passes with the hard
mark **exempt** (decision 5), and the convergence condition — a pass returning
`after_bytes > 0.9 × before_bytes` (`PRODUCTIVE_RECLAIM_RATIO`, decision 6) suspends
auto-enqueueing and logs "budget too small for this workload" **at the soft mark**. It resumes on
its own the next time any pass pays, because `drain_compact_jobs` now returns its
`CompactionRecord`s and the reactor folds *every* pass back into the driver — an operator's
productive pass lifts the suspension just as the driver's own would.

**Slice 3 — bounded defaults.**
`DEFAULT_MAX_SAMPLES` (100,000) is enforced rather than advisory for series with **no policy
record**; a record carrying `max_samples: 0` is honoured as genuinely unbounded (decision 9 —
policy-record *existence* decides, which is the promise the shipped `over_cap_warning` text already
made). `ingest_dead_letter` gained a 30-day horizon (`DEAD_LETTER_KEEP_MS`) pruned by the existing
per-workspace GC pass (decision 7). No early-execution tightening pass was built (decision 10).

## The pause measurement — the slice-2 gate

The deferral in `online-compaction-scope.md` OQ5 was pending a pass measured at budget scale; the
only figure on record was `duration_ms: 22` on a 58 KB log. Measured here on a real SurrealKV store
built through the real write path (`crates/store/tests/compaction_pause_measure_test.rs`, `#[ignore]`,
release build, NVMe):

```
PAUSE MEASUREMENT: before_bytes=2161424374 (2061 MiB) after_bytes=16903331 (16 MiB)
                   duration_ms=771 wall_ms=898 segments_before=5 rounds=128
```

**771 ms to compact a 2.06 GiB log down to 16 MiB — a 128× reclaim.** Sub-second at a scale that is
already past a typical Pi-class budget's soft mark, and the pause scales with the *live set* being
rewritten (16 MiB here), not with the log being discarded.

**Verdict: the auto-trigger is approved.** A sub-second write pause, at most once an hour, is
strictly cheaper than the disk pressure it relieves — the alternative is 2 GiB of dead bytes that
every boot replays. The one-hour minimum interval stays at its scope-suggested value; the
measurement gives no reason to change it. Reproduce with:

```
cargo test --release -p lb-store --test compaction_pause_measure_test -- --ignored --nocapture
```

Caveat worth stating: this is an NVMe SSD. On an SD card the same pass rewrites 16 MiB at maybe
10–20 MB/s, so expect ~1–2 s rather than 0.77 s — still bounded by the live set, still comfortably
inside "cheaper than the pressure". The number to watch on slower media is `duration_ms` on the job
record, which every pass already reports.

## The live run (skill-doc grounding)

The whole flow was also driven end to end on a real booted node with the **real**
`spawn_store_compact_reactors` (tick shortened 30 s → 1 s), budget `8388608` (soft 6710886 / hard
7969177), writing real records until each mark was crossed. Captured in
`docs/skills/store-compact/SKILL.md`; the observed sequence:

```
soft mark crossed  log_bytes=6733387  → enqueued store.compact (system:store-budget)
                   pass: before=6734316 after=436237 duration_ms=76
hard mark crossed  log_bytes=8039608  → compacted exempt from the interval
                   pass: before=8040259 after=437040 duration_ms=83
unproductive run   before=6741738 after=6741889 → "budget too small for this workload",
                   total store-compact jobs = 1 (and never another)
```

That last block is the convergence condition firing on a real store rather than a unit fixture:
one job, then the log line, forever. `free_disk_bytes` was observed as `None` in every status dump,
as documented below.

## Decisions changed from the scope

None. Every decision 1–10 shipped as written. Two things the scope left implicit were resolved:

- **A suspended driver stays suspended at the hard mark too.** The scope says the hard mark
  compacts exempt from the *interval*; it does not say what happens when convergence has already
  concluded that passes reclaim nothing. Ruling: the suspension wins. A pass that reclaims nothing
  at 80% reclaims nothing at 95%, and the exemption exists to beat the *clock*, not the convergence
  condition. The "budget too small" line keeps logging. Pinned by
  `a_store_whose_live_set_is_the_budget_stops_auto_enqueueing`, which ticks past the hard mark 100
  times and asserts zero jobs.
- **`free_disk_bytes` ships as `None`.** No filesystem-stat crate (`libc`, `nix`, `fs4`, `sysinfo`)
  is a *direct* dependency of this workspace — all four appear in `Cargo.lock` transitively only —
  and adding one was out of this session's remit. The field, its serialization and its call site are
  in place behind a one-function private seam (`fn free_disk_bytes()` in `store_admin/status.rs`);
  filling it in is a single-function change with no shape churn for any caller. **Open follow-up**,
  because the scope's "compaction needs free space to run" risk is not observable until it lands.
  `Store::dir()` is `pub(crate)` today, so exposing it is a prerequisite.

## Tests

Real store, real bytes, real jobs records, real ingest path — nothing mocked (rule 9).

Mandatory categories:
- **Workspace isolation** — the ingest isolation suites pass **unmodified**
  (`the_cap_never_crosses_the_workspace_wall`, `the_pass_record_is_workspace_scoped`), plus a new
  workspace-wall case on the dead-letter prune. Slice 3 touches the per-workspace GC path, so this
  was the gate, not a formality.
- **Capability deny** — re-asserted: no `store:status:read` ⇒ opaque `Denied` (no budget read);
  no `store:compact:run` ⇒ `Denied` and no job record. The reactor's own pass mints no principal —
  node maintenance below the namespace wall, the same posture as the retention reactor.
- **Hot-reload** — unchanged and green; extensions hold `Arc<Store>` and survive a budget-triggered
  pass exactly as they survive an operator-triggered one.

Named regressions (`crates/host/tests/store_budget_driver_test.rs`, 7 tests):
- *Convergence — the write-outage-forever regression.* A store whose live set is the budget
  compacts once, the pass reclaims ~nothing, and **no second job appears over 200 ticks** spanning
  both marks. Then a productive pass lifts the suspension and the driver enqueues again.
- *Minimum interval + hard-mark exemption*, in one test: inside the hour the soft mark is `Idle`
  while the hard mark still returns `Enqueue { hard_mark: true }`.
- *Eviction grows the log, compaction reclaims it.* Real deletes on a real store; asserts
  `log_bytes` **increases** after the deletes and drops below the pre-delete size after a pass.
  This pins the append-only property the whole ordering rule depends on.
- *Unbudgeted is inert.* 50 ticks at a 64 GiB log with `None` ⇒ zero jobs, `decide(u64::MAX) ==
  Idle`. The upgrade-changes-nothing gate.
- *Quiet store* ⇒ no pass, no job (the `dev-node-cpu-job-scan` lesson).
- *Attribution* — one crossing, one job, `requested_by: "system:store-budget"`, draining through
  the same reactor path an operator's job takes.

Slice 1 (`node/tests/store_budget_config_test.rs`, `crates/host/tests/store_budget_test.rs`,
`crates/store/tests/status_manifest_test.rs`): env parse unset/set/malformed (`4GB`, `-1`, `1.5`,
overflow all warn and fall back without panicking), marks derive at 80/95%, `None` ⇒ 256 MiB and no
marks, `log_bytes == clog + manifest`.

Slice 3 (`crates/ingest/tests/series_default_cap_test.rs`,
`crates/ingest/tests/series_dead_letter_gc_test.rs`): the bounded default, the opt-out, and the
30-day horizon. **The bounded-default test fails with the change reverted** — verified by
neutering `cap_unpoliced` to a no-op (the pre-slice-3 advisory behaviour) and re-running:

```
an_unpoliced_series_is_bounded_by_the_default_cap ... FAILED
  assertion failed: the default cap evicted exactly the overshoot   left: 0  right: 5
max_samples_zero_opts_out_while_an_unpoliced_series_is_capped ... FAILED
  assertion failed: no policy record at all → the default cap applies  left: 100005  right: 100000
```

Both pass with the change in place. Cost note: those two tests take ~200 s in debug because 100k
samples are not mockable — each seeds >100k through the real `write` → `commit_batch` path. That was
judged the right trade for the one change that starts deleting operators' data.

### What was NOT verified

**A full `cargo test --workspace` did not complete in this session, and it is not because of this
change.** Every attempt was killed by the dev machine reclaiming disk mid-run: `rust/target` was
wiped (disk went 582 GB → 381 GB used, `target/debug/deps` emptied), so ~437 test binaries reported
`No such file or directory (os error 2) … never executed`. **Zero suites reported an actual test
failure in any attempt.** The `hello-v2` wasm fixture (`rust/extensions/hello-v2/target/…`) vanished
the same way between commands, which is a separate prerequisite — `make build-wasm` must run before
`cargo test --workspace` or `lb-cli`/`lb-role-gateway` fail to compile their test targets. That
prerequisite is pre-existing on a clean checkout (verified by stashing this branch's changes).

Verified green per-crate: `lb-store` (incl. the manifest sum), `lb-host` (the 7 budget-driver tests
+ the 4 existing store-admin tests), `lb-node` (env parse), `lb-ingest` (118 tests across 21
targets), and `federation` (44). `cargo build --workspace` and `cargo fmt` are clean. **Re-run the
full suite before merging**, on a machine that is not reclaiming disk:
`make build-wasm && cargo test --workspace`.

## Upgrade impact (slice 3 — behaviour change)

See the release note. In one line: **series with no `series_retention` record are now FIFO-evicted
at 100,000 raw samples** (previously kept forever with a warning); the opt-out is to *create* a
record with `max_samples: 0` **before** upgrading, not to leave one absent. `ingest_dead_letter`
rows now expire after 30 days. `lb_ingest::over_cap_warning` is removed (`default_cap_notice`
replaces it) — an API break for embedders. And bytes do not shrink: every eviction appends
tombstones; only a compaction frees the space, which is what slices 1–2 are for.

## Files

**Rust**
- `rust/node/src/config.rs` — `store_budget_bytes` field + `store_budget_bytes_from_env()`.
- `rust/node/src/builder.rs` — `install_store_budget`; budget passed to `reactors::spawn`.
- `rust/node/src/reactors.rs` — `spawn` takes `store_budget_bytes`, threads it to the reactor.
- `rust/crates/host/src/boot.rs` — the `store_budget` install-at-boot seam.
- `rust/crates/host/src/store_admin/marks.rs` — **new**, the pure mark arithmetic.
- `rust/crates/host/src/store_admin/budget.rs` — **new**, the driver (decide / interval / convergence).
- `rust/crates/host/src/store_admin/reactor.rs` — the budget tick + the enqueue; drain returns records.
- `rust/crates/host/src/store_admin/status.rs`, `tool.rs`, `mod.rs`, `src/lib.rs` — report fields, budget-aware verb, re-exports.
- `rust/crates/store/src/status.rs` — `manifest` bytes in the sum.
- `rust/crates/ingest/src/{cap,gc,overflow,lib}.rs` + `dead_letter_gc.rs` (**new**) — slice 3.

**Tests** — `crates/store/tests/{compaction_pause_measure_test,status_manifest_test}.rs`,
`crates/host/tests/{store_budget_test,store_budget_driver_test}.rs`,
`node/tests/store_budget_config_test.rs`,
`crates/ingest/tests/{series_default_cap_test,series_dead_letter_gc_test}.rs`,
`crates/ingest/tests/series_retention_test.rs` (the one test that asserted the old advisory truth,
rewritten to assert the new one).

## Follow-ups

1. **`free_disk_bytes` is `None`** — needs a filesystem-stat dependency decision and `Store::dir()`
   made visible. Until then the "budget close to the physical disk" risk stays invisible.
2. **Per-workspace quotas** remain out of scope; the budget is node-scoped by design.
3. **Slower media** — re-read `duration_ms` from a real SD-card node's job records before assuming
   the 771 ms figure holds there.
