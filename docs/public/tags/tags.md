# Tags — a typed annotation + relationship graph (as built, S9)

The cross-cutting tag service (README §6.11), SurrealDB-native: a tag is a **shared typed node**
(`tag:[key,value]`, composite id — constructed, deduplicated), applying it is a **provenance-carrying
edge** (`entity -> tagged -> tag`), and the one primitive serves **labeling, provenance, lineage, and
classification**. One embedded datastore, no second search/vector system.

## Model

- **Node:** `tag:[key, value]` — `value` is typed (string/number/datetime/…), so range/temporal queries
  are possible. Per-workspace (the wall): the same id in ws-A and ws-B are distinct records.
- **Edge:** `tagged` carrying `at` / `by` / `source` / `confidence` / `expires`. **Edge identity is
  `(entity, tag, source)`** — a same-source re-tag upserts in place; a different source coexists as a
  distinct edge (so a human assertion and an AI inference of the same tag both survive). The tag key/value
  are denormalized onto the edge as `tkey`/`tval` (a row with `in`/`out` drops fields named `key`/`value`).

## Verbs (MCP surface — and nothing else)

`tags.add(entity, key, value, meta?)` · `tags.remove(entity, key, value?)` · `tags.of(entity)` ·
`tags.find(query)`. Each gated by `mcp:tags.<verb>:call` (opaque deny). `find` takes one polymorphic
object — `{ facets: [{key, value?}] }` — dispatched to exact (`key=value`), key-only (`key` present),
or **faceted intersection** (all facets, a per-query graph traversal). There is **no** event-registration
verb (`DEFINE EVENT` is host-internal — a grant can't weaponize write-amplification).

## Required guardrail: the tag-node cap

A **per-workspace tag-node cap** (default 10_000) is enforced on `tags.add`: a new distinct node past the
cap is **denied** (re-using an existing node never counts). Tags are for *dimensions you filter by*, never
high-cardinality values — and ingest's #1 risk (cardinality) depends on this holding.

## Spike-gated add-ons (matrix-driven)

The slice-0 store spike gates these; built where available on SurrealKV:

| Mode | Backed by | Status |
|---|---|---|
| value full-text | `DEFINE ANALYZER` + `SEARCH BM25` | ✓ shipped (`find_text`) |
| semantic / similar | `HNSW` (`<\|K,EF\|>`) | ✓ shipped — **dimension pinned per index, mismatched-dim writes rejected** |
| per-dimension counts | `GROUP BY` | ✓ shipped **per-query** — the materialized `AS SELECT` view defines but does not populate on SurrealKV; faceted intersection counts are per-query always (combinatorial) |

## Guarantees

- **Workspace isolation** — the IDENTICAL `tag:['region','eu']` built in two workspaces does not leak: a
  ws-B `find`/traversal returns zero ws-A edges (tested with the same value on purpose — a different-value
  test would pass even with a leak).
- **Idempotent across re-sync** — deterministic composite ids: the same edge upserts once.
- **Provenance queryable** — filter edges by `source`/`confidence`.

## Consumers

Ingest's `series.find(facets)` is built on `tags.find`, filtered to `series:` entities — the discovery
layer over heterogeneous payloads.

See `scope/tags/tags-scope.md` for the ask and `sessions/tags/tags-session.md` for the build log.
