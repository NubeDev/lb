---
name: tags
description: >-
  Tag any Lazybones entity with `key:value` graph edges and discover entities by faceted query — the
  uniform metadata/relationship layer over heterogeneous records. Add, remove, list, and find tags via
  `tags.*`. Use when a task says "tag an entity/series/doc", "add a label", "find things tagged X",
  "faceted search/discovery", or "call tags verbs". Tags are graph edges in the one SurrealDB (not a
  parallel store) — they give faceted/indexed search AND relationship traversal. This is what makes a
  store of mixed-shape data coherent: discovery happens through the tag graph, not a common payload
  schema. Tag dimensions you filter by; keep high-cardinality values in the payload.
---

# Tagging & discovery (`tags.*`, the graph metadata layer)

Tags are `key:value` **graph edges** on any entity (a series, a document, a job, a channel…), living
in the same multi-model SurrealDB (rule 2 — not a second system). They give faceted, indexed,
full-text search **and** relationship traversal (entity → tag, and onward through the graph). With
heterogeneous records there's no common schema to query by — so discovery happens through the **tag
graph**, and tags are the single source of truth for an entity's dimensions (e.g. the `series`
labels an ingest producer declares become tag edges once per series — see the ingest skill).

The crate is `rust/crates/tags/` (`lb-tags`); the host bridge is `rust/crates/host/src/tags/`.

## 1. Authenticate

```bash
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/login \
  -H 'content-type: application/json' -d '{"user":"user:ada","workspace":"acme"}' | jq -r .token)
```

Capabilities: `mcp:tags.add:call`, `tags.remove:call`, `tags.of:call`, `tags.find:call`. Workspace-
first; a denial is opaque.

## 2. The verbs (over `POST /mcp/call`)

| Verb | Args | Behavior |
|---|---|---|
| `tags.add` | `entity, key, value` (+ provenance, see §4) | Attach a `key:value` edge to `entity`. |
| `tags.remove` | `entity, key, value?` | Remove a tag; `value` absent removes all values for `key`. |
| `tags.of` | `entity` | The tags applied to `entity`. |
| `tags.find` | `facets: [{key, value?}]` | Entities matching ALL facets (intersection). |

`entity` is the tagged record's id. In `tags.find`, a facet with a `value` is an **exact** match; a
facet without one is **key-only** ("has this dimension"); **all facets intersect**.

```bash
# tag a series, then discover by intersection of facets
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -d '{"tool":"tags.add","args":{"entity":"series:node.cpu_temp","key":"host","value":"pi-7"}}'

curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -d '{"tool":"tags.find","args":{"facets":[{"key":"host","value":"pi-7"},{"key":"region"}]}}'
# → entities tagged host=pi-7 AND having any region     → {"entities":[…]}

curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -d '{"tool":"tags.of","args":{"entity":"series:node.cpu_temp"}}'
```

> `series.find` (ingest skill) is the same faceted query specialized to series — same `facets` shape,
> same tag graph beneath.

## 3. What to tag (and what NOT to)

- **Tag dimensions you filter/facet by** — `host`, `region`, `kind`, `unit`, `env`. These are low-
  cardinality and become useful facets.
- **Keep high-cardinality values in the payload**, never as tags — a per-sample id, a free-text note, a
  timestamp. Tagging them explodes the graph.
- **A tag describes the entity, so write it once**, not per event — the ingest path converts declared
  labels to edges once per series (when first seen / when the label set changes), not per sample.
- **The tag-node cap** bounds distinct tag nodes per workspace (cardinality guard) — exceeding it is a
  loud `BadInput` (`tag-node cap exceeded`), not a silent drop.

## 4. Provenance (optional, on `tags.add`)

A tag can carry where it came from and how sure it is — useful when a model or a producer (not a human)
applied it:

- `source` — `human` (default) | `inferred` | `producer` | `system`.
- `confidence` — a float (for `inferred` tags).
- `at` — a logical timestamp (no wall-clock).
- `expires` — an optional expiry timestamp.

## Gotchas

- **Tags are edges in the one datastore** — not a parallel metadata store; discovery is a graph query,
  not a payload scan.
- **`tags.find` facets intersect** — every facet must match; `value` present = exact, absent = key-only.
- **`tags.remove` without `value` clears the whole key** — pass `value` to remove one.
- **Cardinality is capped** — tag dimensions, not identities; over-tagging trips the tag-node cap.
- **Workspace-walled** — tags and finds resolve only within the caller's workspace; ws-B can't see or
  find ws-A entities.
- **Denials are opaque** — a missing cap and an empty result look alike.

## Related

- Series labels + `series.find` (the ingest specialization): `docs/skills/ingest-series/SKILL.md`,
  `docs/scope/ingest/ingest-scope.md`.
- Units-as-tags provenance that `format.quantity` reads: `docs/skills/prefs/SKILL.md`.
- Scope: `docs/scope/tags/`. README §6.11 (the tag service), §3 (one datastore).
- Source: `rust/crates/tags/` (`lb-tags`), `rust/crates/host/src/tags/`.
