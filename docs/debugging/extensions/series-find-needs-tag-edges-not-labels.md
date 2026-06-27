# `series.find` can't discover a series seeded with `labels` — the write path never tags it

- Area: extensions (ingest/tags adjacent)
- Status: documented (worked around in tests; the core gap is a pre-existing, separate fix)
- First seen: 2026-06-27
- Resolved: workaround in place; root fix tracked as a follow-up
- Session: ../../sessions/extensions/proof-panel-session.md
- Regression test: covered indirectly by rust/crates/host/tests/proof_panel_test.rs `seed_series` + ui ProofPanel.gateway.test.tsx (both seed the tag edge explicitly)

## Symptom

`proof-panel`'s page lists series via `series.find` (a tag-graph intersection). To test it, the
first cut seeded a sample with a label (`Sample { labels: {"kind":"temperature"}, … }`) through the
real `ingest_write` + `drain_workspace` path, then queried `series.find` for `kind:temperature`.
The query returned `{"series":[]}` — the seeded series was **not discoverable**, even though the
sample committed fine and `series.latest` could read it.

## Reproduce

1. `ingest_write` a `Sample` carrying `labels: {"kind":"temperature"}`; `drain_workspace`.
2. `series.find` with facet `kind:temperature` → empty.

## Investigation

- `Sample.labels`'s doc comment says *"Producer's raw dimension declaration … Converted to tag edges
  at commit."* — but **no code performs that conversion.** Grepping the whole tree (`ingest`, `host`,
  `gateway`) finds zero call sites that turn `labels` into `tags::add` edges.
- `lb_ingest::commit_batch` (the commit worker) only UPSERTs the `series` row's
  `{series,producer,seq,ts,payload}` via `CONTENT` and deletes the staged row. `labels` is dropped.
- `lb_ingest::write` (staging) likewise never tags.
- So `series.find`, which keeps only `series:`-prefixed entities that carry the queried tag edges,
  has nothing to match — the discovery edges are never created from a sample's labels.
- (Note: `ui/src/features/ingest/IngestView.gateway.test.tsx` has a "filters series by tag facet"
  case asserting the opposite. Given the above, that assertion can only pass if the series is tagged
  by some path other than the sample's `labels` — i.e. it is either currently red or relies on a
  different seam. Flagged for the ingest owner; out of scope for proof-panel.)

## Fix (workaround used here)

The proof-panel tests seed the **discovery tag edge explicitly** through the real tag write path —
`lb_tags::add` (host test) / `lb_host::tags_add` (the test gateway's `/_seed/series` route) — applying
`kind:temperature` to the `series:<name>` entity. This is the edge a producer's labels *should*
eventually produce; seeding it directly keeps the test on the real `series.find` discovery path
without inventing a fake. `proof-panel` consumes the existing verb correctly; it does not own the
label→tag conversion.

## Follow-up (root fix, not done here)

Implement the documented behaviour: at commit, convert each sample's `labels` into `tags::add` edges
on its `series:<name>` entity (idempotent, producer-sourced), so `series.find` discovers a labelled
series with no extra step. Then revisit the IngestView faceted-search assertion. Tracked in the
proof-panel session doc's follow-ups.
