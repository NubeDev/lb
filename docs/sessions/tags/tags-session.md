# Tags — typed annotation + relationship graph (session)

- Date: 2026-06-27
- Scope: ../../scope/tags/tags-scope.md
- Stage: S8 — data plane (durable store + ingest + tagging) (slice 2). See STAGES.md.
- Status: done

## Goal
Upgrade `lb-tags` from a placeholder to the SurrealDB-native graph: shared typed `tag:[key,value]`
nodes + provenance-carrying `tagged` edges, the `tags.add/remove/of/find` verbs (exact + key-only +
faceted intersection), the required per-workspace tag-node cap, and the spike-gated add-ons the slice-0
matrix marked available (full-text, vector, per-dimension counts). Then wire ingest's `series.find`
on top.

## What changed
- `crates/tags` (`lb-tags`), built out from the stub:
  - `tag.rs` — `tag:[key,value]` typed node; `edge.rs` — the `tagged` edge + `Source`/`Provenance`,
    edge identity `(entity, tag, source)`.
  - `add.rs` — UPSERT the node + UPSERT the edge (deterministic id `[entity,key,value,source]`) in one
    tx; `remove.rs`; `of.rs`; `find.rs` (exact/key-only/faceted intersection).
  - `cap.rs` — required per-workspace tag-node cap (deny a new node past the cap; re-using an existing
    node never counts).
  - `entity.rs` — parse `table:id` for two-arg `type::thing` (dotted ids).
  - Spike-gated: `search.rs` (BM25), `vector.rs` (HNSW, dimension pinned + mismatch rejected),
    `counts.rs` (per-dimension counts).
- Host service `crates/host/src/tags/`: the MCP gate, `tags_add/remove/of/find`, and `call_tags_tool`
  (`tags.add`/`tags.remove`/`tags.of`/`tags.find` — and nothing else; event registration stays
  host-internal). Re-exported from `lb_host` (`Source` re-exported as `TagSource` to avoid the registry
  `Source` collision).
- `crates/host/src/ingest/find.rs` — `series.find(facets)` reuses `lb_tags::find`, filtering to
  `series:` entities.

## Decisions & alternatives
- **Edge identity `(entity, tag, source)`** (resolved): same-source re-tag upserts in place
  (`by`/`confidence`/`expires` mutable); a different source coexists as a distinct edge. `(entity, tag)`
  rejected (would let an AI write overwrite a human's attribution).
- **All three spike-gated add-ons built** (matrix: SEARCH ✓, HNSW ✓, materialized-view defines ✓). One
  add-on degraded at runtime — see below.
- **Per-workspace tag-node cap is a decision of this slice** — default 10_000, deny on exceed (a hard
  stop beats unbounded bloat; ingest's #1 risk is cardinality and tags is its only discovery layer).
- **Vector dimension pinned per index, mismatched-dim writes rejected** (resolved) — caller-supplied
  embeddings, no model in core.
- **`tag_counts` is per-dimension only** — faceted intersection is per-query (combinatorial, not
  materialized); the "no scan" claim is scoped to per-dimension, not intersection.

## Tests
`cargo test -p lb-tags` (5 core + 1 isolation + 4 add-ons) and `cargo test -p lb-host --test tags_test
--test tags_isolation_test` — all green (output in STATUS / final verify).
- **Capability deny per verb** — `denies_each_verb_without_its_grant`.
- **Workspace isolation — the specified test**: the IDENTICAL `tag:['region','eu']` constructed in BOTH
  ws-A and ws-B with edges in each; a ws-B find returns ZERO ws-A entities (and vice versa). Same value
  on purpose (a different-value test would pass even with a leak). Store + MCP.
- **Offline/sync** — composite ids make re-tag idempotent (same-source upsert test stands in for the
  re-sync upsert).
- **Index-correctness** — full-text BM25 match; vector nearest-neighbour + dim-mismatch rejection;
  per-dimension counts; provenance queryable by source.

## Debugging
- debugging/tags/relation-drops-key-value-fields.md — a row carrying `in`/`out` silently drops fields
  literally named `key`/`value`; the edge denormalizes them as `tkey`/`tval`. Regression:
  `add_then_of_returns_the_tag`.
- debugging/tags/dotted-entity-id-needs-two-arg.md — `type::thing("series:node.cpu_temp")` mis-parses a
  dotted id; entity links built two-arg `type::thing($tb,$id)`, raw entity stored as `ent` for exact
  round-trip. Regression: `series_find_discovers_by_tags`.
- debugging/tags/hnsw-knn-needs-ef-arg.md — the single-arg `<|K|>` knn operator returns nothing; the
  two-arg `<|K,EF|>` form works (results pre-ordered by distance). Regression:
  `vector_search_returns_nearest_and_rejects_dim_mismatch`.
- debugging/tags/materialized-view-does-not-populate.md — `DEFINE TABLE … AS SELECT … GROUP` defines but
  never populates on SurrealKV (incremental or backfill); counts computed per-query instead. Regression:
  `materialized_counts_are_per_dimension`.

## Public / scope updates
- Promoted to `public/tags/tags.md`.
- Scope open questions: one polymorphic `find` query object (lean taken); vector dim per key (lean);
  tag-node cap 10_000 + deny (decided); typed values in the composite id (kept). `tag_counts`
  materialized-vs-rebuilt — the engine doesn't populate the view, so per-query for now (recorded).

## Dead ends / surprises
- Materialized AS-SELECT views: the spike marked the DEFINE available, but the view never populated for
  edge writes on SurrealKV — a real degrade discovered during the slice, handled by per-query counts (no
  caller change when a future engine populates the view).
- The `find` faceted-intersection count uses `count(array::distinct([tkey,tval]))` grouped by entity.

## Follow-ups
- Switch `count_by_key` back to the materialized view if a future SurrealDB populates it.
- Lineage/relationship verbs (`derived_from`, `produced_by`) — generic `relate` + allow-list (scope
  lean), deferred.
- STATUS.md updated: slice 2 shipped; S8 exit gate marked MET in STAGES.md.
