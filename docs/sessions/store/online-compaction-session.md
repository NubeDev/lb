# Store — online commit-log compaction (session)

- Date: 2026-07-15
- Scope: ../../scope/store/online-compaction-scope.md (issue #67)
- Stage: post-S8 hardening — data-plane operability. See STAGES.md / STATUS.md.
- Status: done

## Goal
Bound the SurrealKV commit log on a *running* node (today it only compacts at boot), make log
growth observable before it becomes a boot-time surprise (`store.status` + threshold advisory),
and ship a `store.compact` admin job — spike first, because the engine may not allow any of it.

## The spike (ran first, per scope — it decided everything)

Probe: `crates/store/tests/spike_online_compaction.rs` (temporary; deleted once findings landed
here). Real engine, real bytes, no mocks. Pinned versions: surrealdb 2.6.5 / surrealkv 0.9.3
(the entire surrealdb 2.x line pins surrealkv ^0.9.1; the rewritten surrealkv 0.21.x is
surrealdb 3.x-only, so no upgrade path inside this scope).

### Q1 — second live handle (scope shape 1): DISQUALIFIED, and not by a lock
surrealkv 0.9.3 has **no directory lock at all**. A second `surrealkv::Store` opens
*successfully* against a directory the live surrealdb handle holds, and `compact()` "succeeds" —
then **every write made through the live handle after that compact was lost** on reopen (10/10
in the probe). Shape 1 fails open, silently: the most dangerous possible answer.

```
SPIKE Q1: second open SUCCEEDED (no directory lock!) — probing safety
SPIKE Q1: compact() on second handle SUCCEEDED
SPIKE Q1 RESULT: after reopen — lost 0/50 pre-compact rows, lost 10/10 rows written
through the LIVE handle after the second handle compacted
```

### Q2 — drop-release timing (decides shape 2's race-freedom)
Dropping the last `Surreal<Db>` clone does **not** release the engine synchronously: the local
engine's router task drains + `kvs.shutdown()`s *after* the drop (surrealdb
`api/engine/local/native.rs`). Measured release (poll `/proc/self/fd` until no fd under the store
dir): **74–240 ms over 10 rounds** (median 160). Deterministically *detectable*, not
deterministic in latency — so the swap job must poll for release, never assume it.

```
SPIKE Q2 RESULT: drop→fd-release over 10 rounds (ms, sorted):
[74, 109, 121, 122, 145, 160, 182, 197, 204, 240] (min 74 / median 160 / max 240)
```

### Q3–Q6 — the ambush: compaction at 0.9.3 EATS post-compaction writes (P0, shipped boot path)
The Q2 integrity check failed — which turned out to be the real story. Localized over four
probes (surrealdb cycles A, no-compaction control B, settled manual-compaction C, pure-surrealkv
minimal D, option sweep, file-level listing):

- **Law:** any write made into a store whose commit log came from a compaction merge is
  **silently lost at the next open**. Data present before the first-ever compaction survives
  forever. 8/8 cycles reproduced it; the no-compaction control lost nothing in 8 cycles; a 1.5 s
  settle changes nothing (not a shutdown race); options don't matter (versions on/off, defaults).
- **Root cause (upstream, surrealkv 0.9.3 `Core::new`):** it opens the commit-log `Aol` *first*,
  then runs `restore_from_compaction`, which deletes `clog/` and renames the merge dir into
  place. The already-open Aol now points at **unlinked inodes** — replay reads the old data
  through stale fds (so the session *looks* healthy), and every append lands in a deleted file.
  The file listing is the smoking gun: after "write k1 + close", **no file in the store grew**.
- **Production impact of the SHIPPED boot compaction** (`open.rs::compact_log`, runs every
  `Store::open`): fresh node → boot 2 compacts (first pass, data survives) → everything written
  after boot 2 is appended through stale fds → **boot 3 loses it all**. Every boot from the
  third onward destroys the writes made since the previous boot. Logged as
  `debugging/store/compaction-merge-eats-next-sessions-writes.md` with a regression test.

```
SPIKE Q4 RESULT (pure surrealkv): k1 present before compact#2: false; ... k0 present: true, k1 present: false
SPIKE Q6 [after k1 write+close]: ["clog/...0.clog:210", "clog/...1.clog:174", ...]   ← nothing grew
```

### Q7 — the fix: complete the merge with a throwaway open before anyone writes
Because the loss mechanism is "merge applied under an already-open log", the workaround is to
make sure **no writing session ever applies a merge**: after `compact()` + `close()`, do a
throwaway `Store::new` + `close()` — it applies the merge physically (delete + rename) while
performing zero user writes, and the *next* open finds no pending merge and appends through
fresh fds. Proven single-shot and over 8 full compact cycles with zero loss (output below).

### Decision — which shape ships
- **Shape 1 (live-handle): rejected** — demonstrated cross-handle data loss (Q1); there is no
  lock to save you, and upstream 2.x exposes no compact passthrough (checked 2.6.5 source: only
  index compaction exists).
- **Shape 2 (mutex-held handle-swap job): SHIPS**, built on (a) the session mutex as the write
  quiesce, (b) fd-poll release detection (Q2), (c) `compact_log` hardened with the Q7
  merge-completion fix (which also fixes the P0 boot path).
- **Shape 3 (restart-to-compact): not needed**, and would have been *unsafe anyway* until the
  Q7 fix — an honest surprise: the fallback was broken, not just slow.
- **Upstream issue:** drafted (see debugging entry) — ordering bug in `Core::new`; fixed line
  is `restore_from_compaction` before `initialize_clog`. Not blocking; 0.21.x rewrote the crate.

## What changed

**Store crate (`crates/store`)**
- `src/compact.rs` (new) — `compact_log` moved out of `open.rs` and hardened with the
  merge-completion rule (the P0 fix): complete any pending `.merge` with a throwaway
  non-writing open FIRST; compact; complete the fresh merge the same way; if completion fails,
  DELETE the `.merge` (dropping a compaction is always safe, leaving one pending never is).
  Plus the **online pass** `compact(&Store)`: hold the session mutex (writes quiesce), swap the
  handle for `Surreal::init()`, drop it, poll `/proc/self/fd` for release (skip the pass on
  timeout — never compact under a live handle), run `compact_log` on `spawn_blocking`, reopen,
  swap back. Returns `CompactionRecord {before_bytes, after_bytes, duration_ms, ok, error}`.
- `src/status.rs` (new) — `status(&Store) -> StoreStatus {persistent, log_bytes,
  segment_count, last_compaction}`: file metadata only, no records, no lock.
- `src/open.rs` — **the session mutex now CARRIES the handle** (`Arc<Mutex<Surreal<Db>>>`
  instead of a plain `db` field + `Mutex<()>`): the same critical section that guards
  `use_ns`+query guards the compaction handle-swap, so a query can never run against a
  half-open engine. `WsGuard` derefs through the owned guard; all data verbs unchanged.
  `Store` gains `path` (None for memory) and the `last_compaction` slot.

**Caps (`crates/caps`)**
- `Action::Run` added to the grammar (`store:compact:run`) — a pass rewrites log bytes but
  never mutates a record, so the broad author `store:*:write` must not imply it.

**Host (`crates/host`)**
- `src/store_admin/` (new; one verb per file) — `status.rs` (`store_status_run` +
  `LOG_ADVISORY_BYTES` = 256 MiB + the pure `over_threshold_advisory`), `compact.rs`
  (`store_compact_enqueue` → durable `store-compact` job), `reactor.rs`
  (`spawn_store_compact_reactors`: drain enqueued jobs + log the advisory; NEVER
  compaction-on-a-tick), `authorize.rs` (two-gate posture like `store_mutate`), `tool.rs`
  (MCP bridge), `error.rs`.
- `tool_call.rs` — `store.status`/`store.compact` in `HOST_NATIVE_EXACT` + dispatch arm.
- `authz/builtin_roles.rs` — ADMIN_ONLY: `mcp:store.status:call`, `mcp:store.compact:call`,
  `store:compact:run` (status's `store:status:read` rides the inherited `store:*:read`).
- `system/catalog.rs` — the two tools listed.

**Node (`rust/node`)**
- `reactors.rs` — `spawn_store_compact_reactors` wired (30 s tick; config-gated like every
  reactor — rule 1).

## Open questions resolved (scope §Open questions)
1. **Does surrealkv `compact()` tolerate a concurrent open handle?** It *accepts* it (no
   directory lock exists at all) and then loses data — worse than refusing. Shape 1 dead.
2. **Does dropping the last `Surreal<Db>` clone release the dir deterministically?** No —
   asynchronously, 74–240 ms observed; release is *detected* (fd poll), never assumed.
3. **Threshold expression:** absolute bytes (256 MiB default) for v1 — a cheap live-set
   estimate doesn't exist; revisit when one does. Rejected: live-multiple (no cheap
   denominator), segment count (512 MiB segments make it too coarse).
4. **What is in the bloated log?** Measured (the OQ4 experiment,
   `store_admin_test.rs::oq4_bloat_composition_experiment`, real ingest path + retention GC):
   12,000 samples with 92% evicted → **17,701,087 B log over a 679,857 B compacted live set
   (26×)** — superseded versions + eviction tombstones fully account for the growth, and
   compaction recovers it. Same curve as the incident's 65× at larger scale/cadence. No
   write-amplification bug on the ingest path; no follow-up scope needed.
5. **Auto-trigger vs operator-triggered:** operator-triggered for release 1 (OQ5 as scoped);
   the reactor only warns. Auto-trigger is a follow-up once pause cost is measured in the field.

## Test output

### Store crate — full suite (isolation gates UNMODIFIED and green)

```
$ cargo test -p lb-store
compaction_test:        3 passed (incl. repeated_compaction_cycles_keep_every_sessions_writes — the P0 regression)
concurrent_ns_test:     1 passed (64 workspaces, UNMODIFIED — the hard gate on the Store handle change)
isolation_test:         2 passed (UNMODIFIED)
online_compaction_test: 5 passed —
  online_pass_shrinks_log_and_keeps_live_set     (2 workspaces, after < before/2, survivors intact)
  concurrent_writers_during_pass_all_land        (16 writers racing the pass; all land, all durable)
  crash_artifacts_reopen_clean_and_keep_writing  (pending .merge + leftover .tmp.merge windows)
  boot_dividend_compacted_copy_opens_leaner      (compacted copy < raw/5 bytes; opens no slower)
  memory_store_refuses_compaction
crash_test:             4 passed (subprocess SIGABRT suite, 143s)
persistent_parity_test, capability_spike_test, capped/create/increment/list/taint/write_locked/rev_probe: all passed
```

Boot-dividend measured line (from the test's own output):
`boot dividend: raw open ~... (raw bytes) vs compacted open ... — compacted < raw/5 bytes` —
see the run below for exact numbers.

### The second ambush — `DEFINE INDEX` leaks the engine (found by the host job-flow test)

The first host run failed exactly where a real node would: `store.compact` jobs came back
`Failed` with "engine did not release … within 30s". Bisected to a **store-level minimal
repro**: any `DEFINE INDEX` (surrealdb-core 2.6.5 `kvs/index.rs` spawns a builder task
holding the transaction factory) keeps the engine's files open FOREVER — measured 120 s after
the last handle drop, and every real workspace defines the jobs `(kind,status)` index. Fixed
by replacing the fd-zero gate with `wait_for_quiesce` (fd-zero fast path + a 2 s
size/mtime-stability fallback over a provably-inert leaked holder). Full story + safety
argument: `debugging/store/define-index-leaks-engine-blocks-release.md`. Regression:
`index_leak_quiesce_test.rs` (red on the old gate, green on the new).

### Revert-check (the P0 fix)

With the merge-completion rule disabled (`if false { … }` around the throwaway open):

```
thread 'repeated_compaction_cycles_keep_every_sessions_writes' panicked:
assertion failed: cycle 2: sentinel s1 written 1 compacting reopens ago must survive
test result: FAILED. 0 passed; 1 failed
```

Fix restored → `test result: ok. 3 passed` (compaction_test). The shrink/online suites
likewise fail with the pass reverted (no shrink, `after_bytes == before_bytes`).

### Boot dividend (measured, from `boot_dividend_compacted_copy_opens_leaner`)

```
boot dividend: raw open 148.9ms (753,993 B) vs compacted open 140.2ms (38,681 B)
```

Compacted copy is ~19× smaller and opens no slower (test-scale store; the incident-scale
ratio was 65×/13 s — same curve).

### Host — store_admin suite (final, all green)

```
$ cargo test -p lb-host --test store_admin_test -- --nocapture
GROUNDING: status before: log_bytes=57308 advisory=none
GROUNDING: store.compact -> job store-compact-01KXJCC803WX7WGVPV3DJXFTJ5
GROUNDING: job record payload: {"requested_by":"user:ada","outcome":{"at_epoch_ms":1784102141166,
  "ok":true,"before_bytes":58230,"after_bytes":11024,"duration_ms":22,"error":null}}
GROUNDING: job done: outcome ok=true before=58230 after=11024
GROUNDING: status after: log_bytes=12464, last_compaction.ok=true
test result: ok. 5 passed; 0 failed; 1 ignored (the OQ4 experiment, run separately)
  status_reads_with_cap_and_denies_without      (deny-test: store:status:read)
  compact_denied_without_cap_and_writes_no_job  (deny-test: store:compact:run — deny is total,
                                                 and store:*:write does NOT imply it)
  compact_job_enqueues_drains_and_records_outcome  (the end-to-end job flow, real node)
  threshold_advisory_fires_only_over_threshold
```

### Store crate (re-run after the quiesce fix)

```
index_leak_quiesce_test: 1 passed (7.7s — the stability-fallback path)
compaction_test:         3 passed   concurrent_ns_test: 1 passed (UNMODIFIED)
online_compaction_test:  5 passed
```

## Open questions resolved
(tracked against the scope's §Open questions as slices land)
