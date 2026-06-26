# Tags — a typed annotation + relationship graph

Status: **TODO** (stub). Promoted from `scope/tags/tags-scope.md` when the slice ships.

The cross-cutting tag service (README §6.11), in its full SurrealDB-native form: a tag is a **shared
typed node** (`tag:[key,value]`), applying it is a **provenance-carrying graph edge** (`RELATE entity
-> tagged -> tag`), and the same primitive expresses **labeling, provenance, lineage, and
classification**. Search modes map to SurrealDB built-ins — exact (composite id / unique index),
key/faceted (graph traversal), full-text (`SEARCH`/BM25), range (typed values), faceted counts
(materialized views), and semantic (`HNSW` vector). One embedded datastore, no second system.

Filled in on ship with: the `tag`/`tagged`/`tag_counts` model + index set, the `tags.add/remove/of/find`
MCP verbs, and the green deny + isolation + offline-replay + index-correctness (exact/facet/text/range/
vector) tests.

See `scope/tags/tags-scope.md` for the ask.
