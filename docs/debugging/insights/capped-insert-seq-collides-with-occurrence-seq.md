# `capped_insert`'s injected `seq` clobbers the occurrence's `seq` → the ring orders wrong

- Area: insights/occurrences
- Status: resolved
- First seen: 2026-07-05
- Resolved: 2026-07-05
- Session: ../../sessions/insights/insights-session.md
- Regression test: `rust/crates/host/tests/insights_test.rs::ring_cap_evicts_oldest_but_count_is_lifetime`

## Symptom

`insight.occurrences` returned rows whose `seq` was a **ULID string** (e.g. `"01J…"`) instead of
the per-insight monotone `u64` the crate assigns, so newest-first ordering by `seq` was wrong and
the ring's "newest-first: the top row is the Nth firing" invariant broke. Some decodes even failed
because the wire `seq` flipped between `u64` and `string` across reads.

## Reproduce

Raise the same dedup_key 4 times with a ring cap of 3, then `insight.occurrences` → the top row's
`seq` was a ULID, not `4`, and the row count/eviction order didn't match "evict oldest to cap".

## Investigation

`Occurrence.seq` is a `u64` (the parent's post-bump lifetime `count` at append — monotone per
insight). The ring is written through `lb_store::capped_insert`, which is the platform's capped-ring
primitive. Reading `capped_insert`'s contract: it injects **two of its own fields** into every
stored body — `cap_key` (the FIFO bucket) and `seq` (a ULID string, the eviction order it owns).
So the stored body had **two `seq` fields**: ours (`u64`) and theirs (`string`), and on read-back
the ULID won the deserialization, clobbering our monotone counter.

Ruled out: the append logic (the parent's `count` was correct on `insight.get`), the ring cap
(rows were evicting — just ordering wrong), and the occurrence size cap (unrelated). The collision
was specifically on the `seq` field name.

## Root cause

A shared field name (`seq`) between the host record (`Occurrence`) and the store primitive
(`capped_insert`'s own eviction ordering). `capped_insert` owns its `seq`/`cap_key` for FIFO
eviction; the host's per-insight sequence is a different concept that happened to spell the same.

## Fix

`rust/crates/insights/src/occurrence.rs:35` — `Occurrence.seq` serializes as **`oseq`**
(`#[serde(rename = "oseq")]`). The wire/store field is `oseq`; `capped_insert`'s injected `seq`
(ULID) and `cap_key` are ignored on decode (extra fields). The ring now orders newest-first by
`oseq` (the parent's lifetime count — strictly increasing, agrees with the ULID eviction order).
The UI/types mirror the wire field (`oseq`, not `seq`) end to end.

Rejected alternative: rename the field in the Rust struct (`pub oseq: u64`). Rejected because the
domain name is "sequence" (`seq`); only the *wire* name needs to dodge the collision. `serde(rename)`
keeps the Rust API readable and moves the dodge to the one place it matters.

## Verification

`cargo test -p lb-host --test insights_test::ring_cap_evicts_oldest_but_count_is_lifetime` —
asserts the top row's `oseq == 4` (the 4th firing) and the ring holds the newest 3 of 4. Green.

## Prevention

`occurrence.rs`'s doc comment names the collision and the dodge explicitly, and the regression test
pins the wire field name + ordering. A `capped_insert` that namespaced its internal fields
(`_cap_seq`) would make the class impossible — flagged as a store-primitive follow-up, not urgent
now that the insight ring dodges it.
