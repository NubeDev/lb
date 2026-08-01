# Boot took the whole machine down: the store's boot compaction pass OOM-killed sshd, twice, and then restart-looped

- Area: store (boot path — `crates/store/src/open.rs::open` → `compact.rs::compact_log`)
- Found: 2026-08-01, in the field on a Rubix Compute (armv7, 959 MB RAM). Issue
  [#128](https://github.com/NubeDev/lb/issues/128).
- Severity: **P0 availability** — not "the node died": the *box* died, twice, each time costing a
  site visit because ssh was unreachable.
- Status: fixed (boot memory guard, session
  [`sessions/store/boot-memory-guard-session.md`](../../sessions/store/boot-memory-guard-session.md))
  + regression tests.

## Symptom

The node restart-loops and the machine becomes unreachable. `journalctl` after a power cycle shows
the kernel OOM killer firing during node boot and picking **`sshd`** (and whatever else was
resident), not only the node. `Restart=on-failure` then re-ran the identical boot every ~5 s, so
every recovery attempt re-created the same memory spike: the box stayed dark until someone drove to
it. Observed twice on 2026-08-01 on the same unit.

Numbers from the incident box: 959 MB RAM, `MemAvailable` ≈ 802 MB, commit log 617 MB, boot peak
**879 MB anon-RSS**.

## Root cause

`Store::open` ran a **full compaction pass unconditionally** and then opened SurrealDB, which
replays the log again:

1. `compact_log` opens the log and `surrealkv::Store::compact()` resolves **every live value from
   disk into memory** to write the merge set — peak on the order of the live set itself;
2. the merge-completing throwaway open replays the log again;
3. `Surreal::new::<SurrealKv>` replays it a third time to build the in-memory index.

Nothing on that path knew how much RAM the machine had, so a store whose **live set** was 2.4× the
256 MiB advisory drove a boot spike larger than free memory. Two aggravating facts made it fatal
rather than merely slow:

- the 617 MB segment was itself compaction *output* (the log **was** the live set), so the pass that
  killed the box was **guaranteed by construction to reclaim nothing** — the most expensive possible
  no-op, repeated every boot;
- the kernel OOM killer is **global**. Without a `MemoryMax` cgroup on the unit, the victim is chosen
  machine-wide, so a node that over-allocates takes down the operator's only way in.

The disk half of the problem ([#122](https://github.com/NubeDev/lb/issues/122),
`scope/store/disk-budget-scope.md`) would **not** have prevented this: it bounds bytes on disk, the
live set was already over its advisory, and boot ran the pass anyway.

## Fix

`scope/store/boot-memory-guard-scope.md`, all three slices:

1. **The boot pass is preconditioned** (`crates/store/src/boot_guard.rs`, pure functions):
   skip when `log_bytes > BOOT_COMPACT_MEM_RATIO (0.5) × MemAvailable`, and skip when the
   **persisted** last pass was unproductive (`after > 0.9 × before`) and the log has not grown past
   `REGROWTH_RERUN_RATIO (1.25) × last.after_bytes`. Every skip logs at **warn** with all three
   numbers and lands in the `CompactionRecord.skipped` field that `store.status` serves.
2. **A hopeless open is refused, not attempted**: `log_bytes > OPEN_GUARD_MEM_RATIO (1.0) ×
   MemAvailable` ⇒ `StoreError::WontFit`, which names both numbers, the override and the remedies;
   `lb-node` exits nonzero with it on stderr and **never** falls back to `mem://`. A restart loop then
   costs a `stat`, not 90 s of allocation — sshd survives and the box stays reachable.
3. **The pass record is persisted** (`<store dir>/../last-compaction.json`, atomic tmp+rename), so a
   skip decision survives the restart at which it matters and the #122 budget driver re-seeds from it.

Both guards **fail open** when `/proc/meminfo` is unreadable, and the open guard is overridable with
`LB_STORE_OPEN_UNGUARDED=1` — a heuristic that can brick a valid boot would be worse than the bug.

On the incident box the shipped behaviour is: the pass is skipped (617 > 0.5 × 802), the open is
**allowed** (617 < 1.0 × 802), and the node serves on the uncompacted log. Degraded boot speed, zero
drama.

## Regression tests

`crates/store/tests/boot_memory_guard_test.rs` — real SurrealKV stores; only the RAM *number* is
injected into the real pure functions:

- `skip_is_loud_and_recorded_and_the_node_still_opens` — the pass is declined, the WARN line with
  all the numbers really is emitted, `store.status` serves the reason, and the node reads fine.
- `guard_refuses_and_the_override_still_opens` — `WontFit` names both numbers + the remedies, and
  the same directory opens with the override (nothing was touched by the refusal).
- `merge_completion_survives_a_skipped_pass` — the P0 from
  [`compaction-merge-eats-next-sessions-writes.md`](compaction-merge-eats-next-sessions-writes.md)
  still holds under a skip: a genuinely pending `.merge/` is completed **first**, and writes made by
  that session survive the next open.
- `unmeasurable_ram_fails_open`, `persisted_record_roundtrips_and_degrades`,
  `sidecar_is_outside_the_budget_arithmetic`.

`node/tests/store_open_guard_config_test.rs::boot_fails_loudly_when_the_store_will_not_fit_and_the_override_boots_it`
— boot fails with the diagnostic rather than OOMing, and never silently serves an empty `mem://`
store.

**Revert-checked:** gutting `boot_compaction_skip` reddens the three skip tests; gutting
`open_would_not_fit` reddens the two refusal tests.

## Lessons

- **A boot path that allocates proportionally to stored data is a machine-level hazard, not a node
  one.** The kernel's OOM killer is global; the blast radius of "the node used too much" is
  everything else on the box, including the operator's way in.
- **The most expensive pass is the one that reclaims nothing** — and "the log is the live set" is a
  *stable* state, so that pass repeats every boot forever. Any expensive maintenance step needs a
  memory of whether it paid last time, and that memory must survive the restart.
- **A supervisor stanza and an in-process guard are not substitutes.** `MemoryMax` + `OOMPolicy=stop`
  keeps the box alive; only the node can say "your store needs 617 MB and you have 802 MB free".
  Both belong deployed (the unit stanza is tracked in the rubixd repo).

## Cross-links

- Scope: [`scope/store/boot-memory-guard-scope.md`](../../scope/store/boot-memory-guard-scope.md)
  (issue #128) · Session:
  [`sessions/store/boot-memory-guard-session.md`](../../sessions/store/boot-memory-guard-session.md)
- The disk half: [`scope/store/disk-budget-scope.md`](../../scope/store/disk-budget-scope.md) (#122)
- The P0 every skip path must preserve:
  [`compaction-merge-eats-next-sessions-writes.md`](compaction-merge-eats-next-sessions-writes.md)
- Operating it: [`skills/store-compact/SKILL.md`](../../skills/store-compact/SKILL.md)
