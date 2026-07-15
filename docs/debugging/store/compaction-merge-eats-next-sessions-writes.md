# Boot compaction silently destroys every write made since the previous boot (from boot 3 on)

- Area: store (SurrealKV commit-log compaction, `crates/store/src/open.rs::compact_log`)
- Found: 2026-07-15, during the online-compaction spike (issue #67,
  `sessions/store/online-compaction-session.md`)
- Severity: **P0 data loss** in a shipped path — not theoretical, deterministic 8/8 cycles.
- Status: fixed (merge-completion throwaway open in `compact_log`) + regression test.

## Symptom

A store that has been compacted at least once loses **all writes made after that compaction**
at the next open. On a real node: fresh install → boot 2 runs the first compaction (fine) →
everything written after boot 2 vanishes at boot 3. Reads *during* the doomed session look
perfectly healthy — the loss is only visible after the next open. Nothing errors, ever.

## Root cause (upstream: surrealkv 0.9.3, `Core::new` ordering)

`surrealkv::Store::compact()` does not rewrite the log in place: it writes the live set into a
`.merge/` directory, and the swap (delete `clog/`, rename the merge clog in) happens at the
**next** `Store::new` via `restore_from_compaction`. The bug is the order inside `Core::new`:

1. `initialize_clog` — opens the append-log with fds on the **old** `clog/` files;
2. `restore_from_compaction` — deletes `clog/` and renames `.merge/clog` into place;
3. replay + all subsequent appends go through the **stale fds from step 1**: replay reads the
   old (deleted) files so the session looks intact, and every append lands in an unlinked
   inode — gone when the process closes.

Smoking gun (spike Q6): after "write k1 + close" on a post-merge store, **no file in the store
directory grew**. Minimal pure-surrealkv repro (no surrealdb involved): open+put k0+close;
open+compact+close; open+put k1+close; open → k1 is gone, k0 intact. Options don't matter
(versions on/off, thresholds, defaults). Not a shutdown race (persists with 1.5 s settles).

Why nobody upstream hit it: vanilla surrealdb 2.x never calls `compact()` — lb is the unusual
caller via the direct `surrealkv` dep (`open.rs` doc comment records why). surrealkv 0.21.x
(surrealdb 3.x-only) rewrote the crate.

## Fix

`compact_log` now guarantees **no writing session ever applies a pending merge**:

- before compacting: if a `.merge/` is pending from an earlier interrupted run, complete it
  with a throwaway open+close first;
- after `compact()`: immediately do a throwaway `Store::new` + `close()` — it applies the merge
  (delete + rename) while performing zero user writes; the next (real) open finds no pending
  merge and appends through fresh fds;
- if the throwaway open fails, the fresh `.merge/` is **deleted** — dropping a compaction is
  always safe (the old log is untouched until a merge applies); leaving one pending is not.

Fixed line upstream would be: run `restore_from_compaction` *before* `initialize_clog`
(drafted upstream issue text lives in the session doc).

## Regression test

`crates/store/tests/compaction_test.rs::repeated_compaction_cycles_keep_every_sessions_writes`
— 8 open→write→close cycles through `Store::open` (each open compacts); every cycle's sentinel
must survive all later cycles. **Fails before the fix** (loses every post-first-compaction
sentinel, the exact production symptom), passes after.

## Cross-links

- Session: `sessions/store/online-compaction-session.md` (spike Q3–Q7: localization, control,
  root cause, workaround proof)
- Scope: `scope/store/online-compaction-scope.md` (issue #67)
- Related prior art: `open.rs` doc comment ("surrealdb 2.x exposes no path to compact()")
