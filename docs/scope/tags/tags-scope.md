# Tags scope — a typed annotation + relationship graph, SurrealDB-native

Status: scope (the ask). Promotes to `public/tags/` once shipped. Stage: the cross-cutting tag
service (README §6.11) is a **core crate** (`lb-tags`) used from S2 onward; this scope upgrades it
from "key:value strings" to the **best design SurrealDB allows** — landed as one slice (full graph +
full-text + vector + materialized facet views together), tracked under the S8 ingest work that leans
on it.

The name stays **"tags"** (familiar; already in README §6.11 + the core-crate list). But a tag here is
**not a string** — it is a **shared, typed node** in the graph, and *applying* a tag is a **graph edge
that carries provenance**. One subsystem then serves four jobs at once: **labeling**, **provenance**,
**lineage**, and **classification** — the connective tissue that makes a store of heterogeneous data
(the ingest `series`, inbox items, files, jobs, extensions) navigable by **meaning and relationship**
instead of by schema.

**The cut line (this is the real shape of the slice, not "everything at once").** The headline "leverage
SurrealDB 100%" is the *target*, but two capabilities depend on engine features the
`scope/store/persistent-backend-scope.md` spike has not yet verified. So the slice is structured as:

- **CORE — ships unconditionally:** the graph model (`tag` nodes + provenance `tagged` edges),
  composite-ID exact lookup, key-only + **faceted traversal**, and the `tags.add/remove/of/find` verbs.
  These depend only on LOAD-BEARING engine features (composite IDs, `RELATE`, namespaces) that, if
  absent, sink all of S8 anyway.
- **SPIKE-GATED ADD-ONS:** value **full-text** (`SEARCH`/BM25), **vector** semantic search (`HNSW`), and
  **materialized facet-count views** (`DEFINE TABLE … AS SELECT … GROUP`). Each ships **only if** the
  store spike marks it available; a ✗ defers that one capability to a follow-up per the spike's recorded
  fallback — the **core is unaffected**. This is the hard gate, consumed before build, so a missing
  `HNSW` can never invalidate the slice mid-flight.

## Goals

- **Tag = a shared typed node.** `tag:[key, value]` (composite record ID) — deterministic, deduplicated,
  constructed not looked-up. Every entity in a workspace points at the *same* node, so traversal is a
  graph hop both directions, never a scan.
- **Apply = a provenance-carrying edge, keyed to keep attributions distinct.** `RELATE entity -> tagged
  -> tag` where the **edge is a record** holding `at`, `by` (principal), `source` (`human` | `inferred` |
  `producer` | `system`), `confidence`, and optional `expires`. **Edge identity is `(entity, tag,
  source)`** — so a human asserting `kind:telemetry` and an agent later *inferring* the same coexist as
  **two edges**, both attributions preserved; a re-tag from the **same source** upserts in place
  (idempotent). Keying on `(entity, tag)` alone would let the AI write overwrite the human's
  `by`/`confidence` — so that weaker key is **rejected** (it silently breaks the distinguish-by-source
  goal). `by`/`confidence`/`expires` are mutable within a given `(entity, tag, source)` edge.
- **Typed values, not just strings.** A tag value may be a `string`, `number`, `datetime`, `geometry`
  (geo-tagging), a **record link** (`tag:['similar_to', series:x]`), or carry a vector facet — so range,
  temporal, geo, and "is-a" queries are possible at all.
- **Every search mode is a SurrealDB built-in** (see the table in Intent): exact, key-only, value
  full-text, range, faceted intersection **with counts**, and **semantic / similar** via a vector index.
- **The same primitive expresses relationships, not only labels** — `produced_by`, `derived_from`,
  `about` — so provenance and lineage reuse the tag edge model rather than inventing a parallel one.
- **Generic + reusable** — usable on any entity (records, files, messages, inbox items, extensions,
  jobs, **series/samples**), exposed as MCP verbs every extension and the UI call identically.

## Non-goals

- **No second store / search engine.** No Elasticsearch/Meilisearch/standalone vector DB — SurrealDB's
  own `SEARCH` and `HNSW` indexes only (rule #2). The whole point is 100% SurrealDB.
- **No free-text taxonomy management UI / ontology engine.** This is the storage + query primitive; a
  curated-vocabulary admin flow (if ever wanted) is a later, separate scope.
- **No embedding generation in core.** A vector tag's embedding is *supplied* by the caller (e.g. the AI
  gateway produces it); `lb-tags` stores and indexes it. It does not call a model.
- **No bypass of the capability wall via DB row-permissions.** We do **not** lean on SurrealDB
  `PERMISSIONS` for tenancy — the host capability gate + namespace-per-workspace remain the wall
  (capability-first). DB-level constraints are belt-and-braces at most.
- **No change to the SDK/WIT boundary.** Tag verbs are host MCP tools like any other.

## Intent / approach

**A label is a node; applying it is a typed edge.** The weak design (`entity.tags = ["unit:celsius"]`,
strings in a column) cannot traverse, cannot carry provenance, cannot type its values, and forces a
scan for "everything tagged X." The SurrealDB-native design fixes all four:

```surql
-- The shared tag node: composite ID = deterministic + deduplicated. Constructed, never looked up.
tag:['unit','celsius']
tag:['region','eu-west']
tag:['temp_threshold', 80]          -- a TYPED (numeric) value

-- Applying a tag is a RELATE edge; the edge is a record carrying provenance.
RELATE series:node_cpu_temp -> tagged -> tag:['unit','celsius']
    SET at = time::now(), by = $principal, source = 'producer', confidence = 1.0;

-- Traverse both directions in the graph — no scan:
SELECT * FROM tag:['region','eu-west']<-tagged<-series;   -- all series in eu-west
SELECT ->tagged->tag.* FROM series:node_cpu_temp;          -- all tags of one series
```

**Every search mode maps to a SurrealDB feature — this is the 100%-leverage table:**

| Query mode | SurrealDB feature used |
|---|---|
| Exact `key:value` | composite record-ID lookup / `DEFINE INDEX UNIQUE` |
| key-only ("has any `region`") | graph traversal `->tagged->tag WHERE key = 'region'` |
| value full-text | `DEFINE ANALYZER` + `DEFINE INDEX … SEARCH BM25` on `tag.value` |
| range (numeric / temporal) | typed value + standard index, `WHERE value > 80` |
| per-dimension counts ("series per region") | `DEFINE TABLE tag_counts AS SELECT count() … GROUP` (materialized) |
| faceted **intersection** ("eu-west AND telemetry") | graph traversal per query — combinatorial, **not** materializable (see risk) |
| semantic / "similar to" | `DEFINE INDEX … HNSW` on a vector tag value |
| live updates to a tag set | `LIVE SELECT` (a store feature; motion still rides Zenoh) |
| auto-derived tags | `DEFINE EVENT` triggers (use sparingly — see risks) |

**Relationships are the same primitive.** Because the edge is `RELATE`, `tagged` generalizes:
`series -> produced_by -> principal` (provenance), `sample -> derived_from -> sample` (lineage),
`doc -> about -> tag` (classification). One model, four jobs — and the reason heterogeneous data stays
coherent: you query by relationship/metadata, not by a payload schema that doesn't exist across types.

**Rejected alternatives:**
- *Strings in a column.* Rejected — no traversal, no provenance, no typing, scan-bound. The status quo.
- *A separate search/vector service.* Rejected — violates one-datastore; SurrealDB does both natively.
- *DB row-level permissions for tenancy.* Rejected as the *wall* — capability-first + namespace is the
  wall; DB perms are at most a redundant check.
- *Auto-tagging everything via `DEFINE EVENT`.* Rejected as a default — triggers that fan out on every
  write are a hidden cost and a debugging trap; keep derivation explicit, allow events only narrowly.

## How it fits the core

- **Tenancy / isolation:** tag nodes **and** edges live in the workspace namespace; a ws-B traversal
  physically cannot reach a ws-A tag (the hard wall, structural). Tag nodes are **not** shared across
  workspaces — `tag:['region','eu']` in ws-A and ws-B are distinct records in distinct namespaces.
- **Capabilities:** `mcp:tags.add:call`, `tags.remove`, `tags.of`, `tags.find` gate every operation;
  deny is opaque (no existence signal — `tags.find` without the grant cannot enumerate). The mandatory
  deny-test covers each verb.
- **Placement:** `either` — it is a core crate compiled into every node; no role-specific behavior. A
  tag written on an edge node syncs to the hub like any record.
- **MCP surface:** `tags.add(entity, key, value, meta?)`, `tags.remove(entity, key, value?)`,
  `tags.of(entity)`, `tags.find(query)` — where `query` supports exact/key/range/facet/text/vector modes,
  each backed by the matching index above. The universal contract: extensions, UI, agents call it identically.
- **Data (SurrealDB):** `tag` (SCHEMAFULL node table, composite ID `[key,value]`), `tagged` (a
  `TYPE RELATION` edge table with the provenance fields), `tag_counts` (a materialized `AS SELECT … GROUP`
  view), plus the `DEFINE INDEX` set (unique exact, `SEARCH` full-text, `HNSW` vector). All **state**.
- **Bus (Zenoh):** none directly — tags are state. A "tags changed" notification, if needed, is ordinary
  motion published by the caller, not by this crate (state vs motion stays clean). `LIVE SELECT` is an
  optional store-side convenience, not a substitute for the bus.
- **Sync / authority:** tag nodes and edges are `(table, id)` records that sync on the existing §6.8
  path; the deterministic composite IDs make tagging **idempotent across a re-sync** (the same edge id
  upserts once). Offline tagging buffers and replays like any write.
- **Secrets:** none.

## Example flow

A series gets labeled, classified, and made semantically searchable:

1. The ingest producer tags its series: `tags.add(series:node_cpu_temp, "unit", "celsius",
   {source:"producer"})` and `tags.add(series:node_cpu_temp, "region", "eu-west")`. Two `tagged` edges
   to two shared `tag` nodes, each carrying who/when/source.
2. A triage agent classifies it: `tags.add(series:node_cpu_temp, "kind", "telemetry",
   {source:"inferred", confidence:0.92})` — the **edge records it was AI-inferred**, distinguishable
   from the producer's declarations.
3. A dashboard runs **faceted discovery**: `tags.find({region:"eu-west", kind:"telemetry"})` → a graph
   intersection over `tagged` edges, returning the series — *without knowing their payload shapes*. The
   `tag_counts` view answers "how many series per region" with no scan.
4. The UI search box runs **full-text**: `tags.find({text:"celcius"})` → the `SEARCH` BM25 index matches
   the fuzzy value.
5. A "find similar series" feature runs **vector**: `tags.find({similar_to:<embedding>})` → the `HNSW`
   index returns nearest neighbors over a supplied embedding tag.
6. **Lineage** is the same edge model: `RELATE rollup_series -> derived_from -> series:node_cpu_temp` —
   "where did this rollup come from?" is one traversal.

No payload schema was consulted at any step; the graph carried all the meaning.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md`:

- **Capability deny** — each verb (`add`/`remove`/`of`/`find`) refused without its grant; `find` cannot
  enumerate or leak tag existence without `tags.find`.
- **Workspace isolation — the highest-risk wall case here, so the test is specified, not generic.**
  Because `tag:[key,value]` is the *same constructable record ID in every namespace*, isolation rests
  entirely on `use_ns` scoping — a single forgotten one leaks across tenants. The test must therefore:
  construct the **identical** `tag:['region','eu']` in **both** ws-A and ws-B, write `tagged` edges in
  each, then assert a ws-B `tags.find`/traversal returns **zero ws-A edges** (and vice versa). A test
  that uses two *different* tag values would pass even with a leak — so it is explicitly disallowed.
  Across **store + MCP**.
- **Offline / sync** — tagging offline replays idempotently (the composite edge id upserts once across a
  re-sync; no duplicate edges).

Plus the index-correctness cases for the **spike-gated add-ons** (run only for the features the store
spike marked available):

- **Exact / key / faceted** traversal returns the right entity set; faceted intersection of N tags is
  correct; `tag_counts` matches a ground-truth count.
- **Full-text** `SEARCH` matches tokenized/fuzzy values; **range** queries on numeric/temporal tag
  values are correct; **vector** `HNSW` returns the expected nearest neighbors for a known embedding.
- **Provenance** is recorded and queryable (filter `tagged` edges by `source`/`confidence`).

## Risks & hard problems

- **Embedded-engine feature availability — verify FIRST.** Our current build is `kv-mem` (in-memory),
  and **buckets already surprised us by being unavailable there** (S4 note). Before building the
  spike-gated add-ons, confirm `SEARCH` (full-text) and `HNSW` (vector) indexes are supported in our
  embedded engine — and this **depends on the persistent-backend swap** the ingest scope flagged
  (`Store::open(path)` on `surrealkv`/rocksdb). If a feature is unavailable, that capability degrades to
  a follow-up; the core graph + exact/facet must not. **Spike this on day one.**
- **Cardinality — a load-bearing cap, not "possibly".** Tags are for **dimensions you filter by**
  (`region`, `kind`, `unit`), never high-cardinality values (a raw reading, a UUID) — those belong in the
  payload. This is **not optional discipline**: ingest names cardinality explosion as *its* #1 risk and
  uses tags as its **only** discovery layer over heterogeneous payloads, so the primary consumer's
  robustness depends on it. A **per-workspace tag-node cap is therefore a decision of this slice** (with
  a documented guideline + a deny/warn when exceeded), not a future maybe.
- **Vector cost.** `HNSW` index build/memory is non-trivial; gate vector tags behind an explicit value
  type so most tags stay cheap scalar nodes. Don't vector-index by default.
- **Vector dimension/model must be pinned, or the second caller corrupts the index.** Embeddings are
  caller-supplied (no model in core), but `HNSW` requires **fixed dimensionality** — two callers, or two
  AI-gateway model versions, supplying different-dim vectors to the same workspace's vector tag mismatch
  or corrupt the index. So **dimension (and ideally model id/version) is declared at index-definition
  time**, and a mismatched-dim write is **rejected**, never stored. This is a correctness bug if left
  implicit, not a tuning concern.
- **`tag_counts` is per-dimension only.** A materialized `GROUP` view gives "series per region" cheaply;
  arbitrary multi-tag **intersection** counts ("eu-west AND telemetry") are combinatorial and **computed
  per query** (a graph traversal cost), not free from a view. The "no scan" claim applies to
  per-dimension counts, not intersections — don't oversell it.
- **`DEFINE EVENT` fan-out.** Auto-derivation triggers are a hidden write-amplification and debugging
  trap; keep them narrow and explicit, never a blanket "tag everything." **Event registration is
  host-internal only** — there is **no caller-facing verb** for it (the MCP surface is add/remove/of/find
  and nothing else), so a grant can never weaponize write-amplification.
- **Edge dedup semantics (resolved).** Edge identity is `(entity, tag, source)` — a re-tag from the same
  source upserts in place (idempotent, `by`/`confidence`/`expires` mutable); different sources coexist as
  distinct edges (the distinguish-by-source goal). Keying on `(entity, tag)` alone is rejected.

## Open questions

**Resolved by the shipped slice (2026-06-27) — see `sessions/tags/tags-session.md`:**

- **Value typing in the composite ID:** typed element in the array id kept (`tag:['temp_threshold', 80]`)
  — the value rides the composite id; range queries land as a follow-up but the typing is in place.
- **Vector dimension declaration:** **per vector-tag key**, pinned at index-definition; a mismatched-dim
  write is rejected. Lean taken.
- **Tag-node cap value:** **10_000 per workspace, deny** on exceed (a decision of this slice).
- **`tags.find` query grammar:** **one polymorphic object** (`{ facets: [{key, value?}] }`) dispatched to
  exact/key-only/faceted. Lean taken.
- **Materialized-view refresh:** **per-query** — the `AS SELECT … GROUP` view *defines* on SurrealKV but
  does not *populate* (incremental or backfill), so `count_by_key` computes per-query
  (debugging/tags/materialized-view-does-not-populate.md); `define_counts_view` is the seam to switch back
  when an engine populates it.

**Still open (deferred follow-ups):**

- **Cross-entity relationship verbs** (`produced_by`/`derived_from`): generic `relate(a, edge, b)` +
  allow-list (lean) — not built this slice.
- **Namespace vs shared vocabulary:** per-workspace confirmed (the wall); a curated shared vocabulary
  stays out of scope.

Resolved in this doc (no longer open): **edge identity is `(entity, tag, source)`** (multiple
attributions coexist; same-source re-tag upserts); the slice is **core-unconditional + spike-gated
add-ons** (not "everything at once"); a **per-workspace tag-node cap** is a decision of this slice;
vector tags **pin dimension/model** and reject mismatches; `tag_counts` is **per-dimension only** (not
intersection); `DEFINE EVENT` is **host-internal, never a caller verb**.

## Related

- README **§6.11** (the tag service — this scope is its full design), **§6.1** (SurrealDB multi-model:
  graph + full-text + vector + views, the engine this leans on), **§6.8** (sync/authority),
  **§3** (one datastore, state vs motion, capability-first, the wall).
- `scope/ingest/ingest-scope.md` — the primary new consumer: tags are the label + discovery layer over
  heterogeneous `series`, and lineage (`derived_from`) for rollups.
- `scope/inbox-outbox/` — inbox items already carry tags (`source:github`, `needs:triage`); this upgrades
  the model underneath them.
- `scope/store/` — the SurrealDB record/index model this extends; the persistent-backend swap it depends on.
- `scope/files/`, `scope/jobs/`, `scope/extensions/` — other taggable entities.
- `scope/agent/` + `scope/ai-gateway/` — the supplier of vector-tag embeddings (out of this crate).
