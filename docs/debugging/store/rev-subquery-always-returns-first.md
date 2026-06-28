# Store-managed `rev` always stayed at 1 (the monotonic bump never incremented)

- Area: store
- Status: resolved
- First seen: 2026-06-28
- Resolved: 2026-06-28
- Session: ../../sessions/undo/undo-build-session.md
- Regression test: rust/crates/store/tests/rev_probe_test.rs
  (`rev_starts_at_one_and_increments_monotonically`, `rev_is_per_record_not_global`)

## Symptom

The first probe of the new store-managed `rev` (the optimistic-concurrency token the undo journal's
conditional restore tests against) failed: every write reported `rev = 1`, never 2, 3, ….

```
assertion `left == right` failed: second write bumps to rev 2
  left: 1
 right: 2
```

`read_versioned` itself worked (a fresh write read back `rev = 1`); only the *bump* was broken.

## Reproduce

The first `write` UPSERT derived the new rev from a sub-SELECT over the record being upserted:

```sql
UPSERT type::thing($tb, $id) CONTENT {
    data: $data,
    rev: (SELECT VALUE rev FROM ONLY type::thing($tb, $id))[0] ?? $first
} RETURN NONE
```

against embedded SurrealDB (`kv-mem`, surreal 2.6). Every write landed at `rev = 1`.

## Investigation

- `SELECT VALUE rev FROM ONLY type::thing(...)` with `FROM ONLY` returns the **scalar** `rev`, not a
  one-element array. Indexing it with `[0]` therefore yields `NONE`, which the `?? $first` coalesce
  turned into `$first` (1) on *every* write — so the prior rev was never actually read.
- Confirmed by isolating the expression: reading the field directly off the record id works and is a
  scalar, no array wrapper needed.

## Root cause

A SurrealQL shape mismatch: `SELECT VALUE … FROM ONLY` is already scalar; the `[0]` index assumed a
result set. The coalesce masked the bug (it always fell through to the first-rev default) instead of
erroring, so the write "succeeded" while silently never incrementing.

## Fix

Read the prior rev by **field access on the record id**, which is a scalar and needs no indexing:

```sql
rev: (type::thing($tb, $id).rev ?? ($first - 1)) + 1
```

A new record has no `.rev` → `?? ($first - 1)` (0) → `+ 1` = `$first` (1); an existing record's rev is
read and incremented. Applied identically in the three write seams that stamp rev:

- `rust/crates/store/src/write.rs`
- `rust/crates/store/src/write_tx.rs` (both the change and the effect upsert)
- `rust/crates/store/src/write_journaled.rs` (the new atomic before-image seam)

## Verification

`cargo test -p lb-store` — `rev_probe_test` passes (rev 0 absent → 1 → 2 → 3, per-record independent),
all pre-existing store tests still green (the rev field is forward-compatible: `read` still returns
only `data`; legacy rows default to `rev = 1`). `cargo test --workspace` — 175 test binaries green.

## Prevention

`rev_probe_test` asserts strict monotonic increment and per-record independence, so a regression
(re-introducing the array-index mistake, or breaking the bump) fails loudly. Guardrail: when deriving
a value from a record inside its own UPSERT, prefer scalar field access (`type::thing(...).field`) over
a sub-SELECT — the `FROM ONLY` scalar-vs-set shape is an easy silent footgun in SurrealQL.
