# Ingest

TODO: filled when the features ship. See `docs/scope/ingest/` for the asks
(`ingest-scope.md`, `series-retention-scope.md`, `webhooks-scope.md`,
`drain-backpressure-scope.md`).

## What `ingest.write` guarantees about latency

Shipped 2026-07-15 — the one part of this surface that is settled and worth stating early, because
producers depend on it.

**Your write is never billed for anyone else's backlog.** A sample you push is durably appended to
staging, and the call commits **at most your own batch** (`ceil(your_samples / 256)`) before
returning. Whatever else is queued in the workspace — another producer's burst, a webhook flood, a
federation mirror — is committed by a **background worker**, not inside your call. One producer's
write latency cannot scale with another producer's staging depth.

**Your write is still readable immediately.** The bounded drain preserves the round-trip: a sample
written over a bridge is visible to the very next `series.latest` / `series.read` over that same
bridge, with no explicit drain. That property is deliberate and tested.

**The bound, stated honestly:** if you write more than 256 samples in one call, that call commits
its own work in batches; if a large backlog sits ahead of you, some of your batch budget may commit
those older rows first (staging drains oldest-first). Either way the cost is bounded, and the
background worker commits the remainder within seconds — nothing is stranded, and exactly-once per
`(series, producer, seq)` holds throughout.

Before this shipped, `ingest.write` drained the entire workspace backlog inside the caller's call: a
single sample behind a 4,671-row backlog took 18.5 seconds, and a producer that timed out left the
backlog in place for the next push to hit again.
