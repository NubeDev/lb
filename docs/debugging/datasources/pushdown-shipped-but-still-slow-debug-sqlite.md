# `federation.query` still 3–4 s after statement pushdown shipped — debug-built SQLite

**Date:** 2026-07-06 · **Area:** datasources · **Status: fixed**

## Symptom

The federation-pushdown scope shipped (federated providers + `datafusion_federation::default_session_state()`;
the structural EXPLAIN test proves a multi-table single-source plan collapses to ONE
`VirtualExecutionPlan` with a `base_sql=` covering all four tables). Yet the demo query
(4-table JOIN + GROUP BY over `demo-buildings`, ~956k `point_reading` rows) still took
**3.1 s** through `POST /mcp/call`, vs **0.41 s** in the `sqlite3` CLI.

## False leads (each ruled out with a measurement)

1. **Stale sidecar.** The first re-test WAS against a stale child: the supervised sidecar
   (spawned 18:14:05) predated the rebuilt `target/debug/federation` (18:15:03) —
   `/proc/<pid>/exe` showed `(deleted)`. A supervised native sidecar never picks up a
   rebuilt binary until restarted (same class as the flows "dev node no hot-reload" trap,
   now for sidecars: **check `/proc/<pid>/exe` for `(deleted)` first**). Restarting fixed
   the staleness — but the timing was unchanged, so this wasn't the cause.
2. **Pushdown not firing.** No — host overhead measured ~25 ms (`SELECT 1` end-to-end),
   and the physical plan against the real demo db showed the federated node with a
   `base_sql` essentially identical to the submitted SQL.
3. **Unparser emitting slow SQL.** No — pasting the exact unparsed `base_sql` into the
   `sqlite3` CLI ran in 0.41 s, and `EXPLAIN QUERY PLAN` was **identical** (same
   `SCAN r` + three index SEARCHes) in the CLI and in the bundled rusqlite.

## Root cause

Same SQL, same query plan, **different SQLite binaries**: the sidecar is a **dev-profile
build**, and `libsqlite3-sys` (bundled) compiles `sqlite3.c` at the cargo profile's
opt-level — `-O0` under `[profile.dev]`. An unoptimized SQLite executed the scan-heavy
join in 3.4 s where the (optimized) system CLI took 0.41 s — an ~8× interpreter-level
slowdown that perfectly masked the (working) pushdown. Measured in-crate: the same
statement through `rusqlite::Connection` took 3.45 s before the fix, 0.39 s after.

## Fix

`rust/Cargo.toml`:

```toml
[profile.dev.package.libsqlite3-sys]
opt-level = 3
```

One C translation unit, negligible build-time cost, applies to every dev binary that
links the bundled SQLite (federation sidecar, tests). After rebuild the demo query runs
**0.43 s** through the full federated path (`run_query`) — engine speed, as the
pushdown scope targeted.

## Regression test

None added deliberately: a wall-clock assertion on an optimizer flag would be flaky by
construction, and the *functional* guarantees are already pinned by the pushdown suite
(`query::tests` — exact-answer JOIN/GROUP BY, structural one-federated-scan EXPLAIN,
ROW_CAP, COUNT(*), SELECT-only). The profile override carries a comment explaining why
it must not be removed; this entry is the durable record.

## Lessons

- **A debug-profile C dependency can eat an entire optimization.** Cargo profiles apply
  to `cc`-built C code too; for an embedded database engine the dev/release gap is ~8×.
  When "the fix shipped but it's still slow", compare the SAME statement on the SAME
  file through the SAME library build before blaming the new code.
- **Supervised sidecars don't hot-reload.** After rebuilding a native extension, restart
  it (or the node); `ls -l /proc/<pid>/exe` → `(deleted)` is the one-line staleness check.
- **Bisect by layer with timings, not by reading code:** end-to-end trivial query (host
  overhead) → single-table aggregate (provider path) → the join (the regression) → exact
  unparsed SQL in the CLI (engine) → same SQL via the linked library (build of the engine).
  Each step eliminated a layer with one number.
