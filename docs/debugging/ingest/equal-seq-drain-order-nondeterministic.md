# ingest — equal-`seq` staged samples drain in nondeterministic order

**Symptom (2026-07-14, series-plane readiness):** the series-cardinality-cap test seeded three
different series (`a`, `b`, `c`) all at `seq = 1` with a cap of 2 and asserted `c` was the one
dead-lettered. It failed intermittently: sometimes `c` committed and `a` or `b` was diverted.

**Cause:** the commit worker drains staging `ORDER BY sample.seq ASC, sample.ts ASC` — with equal
`seq` AND equal `ts` across different series, the engine's order among them is unspecified. The
cardinality gate admits new series names in drain order, so *which* new series hits the cap is
nondeterministic under ties.

**Fix:** the test seeds distinct seqs (1, 2, 3) so drain order — and therefore the cap decision —
is deterministic. This is a **test-authoring rule**, not a product bug: the cap's contract is
"at most N distinct series survive; the rest are dead-lettered, never dropped", which never depended
on tie order. Regression test: `series_cardinality_cap_dead_letters_new_series`
(`rust/crates/ingest/tests/series_plane_test.rs`).

**Rule of thumb:** any assertion about *which* row wins a bounded admission (cap, drop-oldest,
batch cut) must seed a total order on the drain key `(seq, ts)`; never rely on tie-breaking.
