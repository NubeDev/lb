# Embeddings scope — doc→vector pipeline and semantic search

Status: scope (the ask). Promotes to `doc-site/content/public/embeddings/` once shipped.

Connect the document store to the HNSW vector primitive that already exists in
`crates/tags/src/vector.rs`, using embedding models served through the ai-gateway contract
(embeddings are already named in that contract, deferred past S5 — this scope pulls them
forward). The ask: a workspace-walled pipeline that chunks markdown docs, embeds the chunks,
and indexes them; plus one search verb that combines metadata filtering with KNN so callers
(the agent, extensions, UIs) can ask "find the docs about X" and get capability-gated hits.
Today embeddings are caller-supplied and vector search is attached only to tags — there is no
doc→vector path at all.

## Goals

- **`Provider::embed` on the ai-gateway contract** — `EmbedRequest { model, inputs[], workspace/actor scope, idempotency_key }` → `EmbedResponse { vectors, dim, model_id, usage }`. One real adapter (OpenAI-compatible `/embeddings`, same wire family as the existing chat adapter) plus a deterministic mock for tests.
- **A doc indexing pipeline in the host** — chunk a markdown doc (heading-aware, size-capped, overlapping), embed chunks via the gateway, upsert into a per-workspace HNSW index keyed `(doc_id, chunk_n)`.
- **A hybrid search verb** — `docs.search { query, filter (tags/owner/time), k }`: metadata filter + KNN, results re-gated per doc read capability before return.
- **Vectors are derived data** — always rebuildable from the markdown source of truth. Wiping the index loses nothing.
- **Model and dimension pinned per index** — recorded in an index-meta record at definition time (the HNSW primitive already rejects dimension mismatches). Changing the model is an explicit re-embed migration run as a job, never a silent swap (ai-gateway scope names this exact rule).

## Non-goals

- **No model in core** — embedding computation stays behind the gateway contract, exactly like completions. Core never links a model runtime.
- **No binary extraction** — PDF/XLSX/email → markdown is the sibling extraction seam (`../document-store/doc-extraction-scope.md`). This scope starts at "a markdown doc exists."
- **No answer synthesis** — RAG-style "retrieve then answer" is the agent's loop consuming `docs.search`; this scope ends at ranked doc/chunk references.
- **Not replacing tag vectors** — `tag_vector` stays; this adds the doc path beside it.
- **No cross-workspace search** — the workspace wall is absolute, as everywhere.
- **Full-text search** — SurrealDB FTS hybrid scoring is a named follow-up, not this ask.

## Intent / approach

Three parts, in dependency order:

1. **Gateway: `embed` beside `complete`.** Extend the stable contract (`role/ai-gateway`)
   with the embedding request/response pair and add `/embeddings` to the OpenAI-compatible
   adapter. Idempotency-cached like completions, so re-indexing a doc whose chunks didn't
   change re-spends nothing (key = model_id + content hash of the chunk).
   *Alternative rejected:* a separate embedding sidecar — that is the "two gateways" failure
   mode the ai-gateway scope explicitly forbids; embeddings are model access like any other.

2. **Host: the indexer.** A host service (`crates/host/src/embeddings/` or under `assets/`)
   with two triggers: an explicit `docs.reindex` **batch job** (bulk import, model migration,
   rebuild) and a reactor on doc write for incremental freshness. Chunking is heading-aware
   with a size cap and overlap; each chunk row carries `(ws, doc_id, chunk_n, content_hash,
   embedding)` in a `doc_vector` table with the HNSW index defined per workspace, dimension
   pinned from the index-meta record. Content-hash keys make the pipeline idempotent: an
   edit re-embeds only changed chunks; deletes cascade.
   *Alternative rejected:* embedding inline inside `put_doc` — couples every doc write to a
   network call and violates the fast-write posture; the reactor keeps writes cheap and the
   index eventually fresh.

3. **Host: the search verb.** `docs.search` runs the workspace-scoped metadata filter and
   KNN (`<|k,ef|>`, as `find_similar` does today), joins hits back to docs, then re-gates
   each result through the same read check as `get_doc` — a hit the caller can't read is
   dropped, never leaked as a snippet. Returns `{ doc_id, title, chunk_n, snippet, score }`.

## How it fits the core

- **Tenancy / isolation:** `doc_vector` rows and the HNSW index live in the workspace
  namespace like every other record; the query embedding is computed under the caller's
  workspace scope. Isolation tested (a doc in ws A must never surface in ws B's search).
- **Capabilities:** `mcp:docs.search:call` to search; results additionally filtered by the
  caller's per-doc read reach (deny = the hit silently drops out). `mcp:docs.reindex:call`
  gated admin-tight since it spends model budget. Embed calls carry the gateway's normal
  budget/policy checks — a workspace with no embed budget gets an honest error, not a hang.
- **Placement:** either. Edge nodes resolve the embed model to a local provider through the
  same gateway contract (local-only flag honored); cloud uses hosted keys. No role branch.
- **MCP surface (§6.1 shapes):** one read verb (`docs.search`) and one batch (`docs.reindex`)
  which **must be a job** — it fans out over N docs × M chunks with network I/O, the
  archetypal long batch. Per-item results (doc id → indexed | failed), resumable, idempotent
  by content hash. No CRUD verbs: chunks/vectors are derived, never caller-written. No live
  feed: index freshness is observable via the job status, not a stream.
- **Data (SurrealDB):** new `doc_vector` table (+ HNSW index per §6.1's vector support) and
  one `embedding_index` meta record per workspace `{ model_id, dim, defined_ts }`. Pure
  state; no motion stored.
- **Bus (Zenoh):** the doc-write reactor trigger is fire-and-forget motion — a missed event
  is healed by the next `docs.reindex` job, so no outbox needed for freshness.
- **Sync / authority:** vectors are derived and node-local; they are **not synced** — each
  node rebuilds its own index from synced docs. This keeps sync payloads small and sidesteps
  cross-node dimension/model drift entirely.
- **Secrets:** none new — provider keys stay sealed inside the gateway (`lb-secrets`), never
  visible to the indexer or callers.
- **No mocks / fake backend:** tests run against the real store and real HNSW index; the one
  permitted fake is the external model provider — the existing `MockProvider` gains a
  deterministic `embed` (hash-derived vectors) in its one named file.
- **SDK/WIT impact:** none — no change to the extension ABI; extensions consume `docs.search`
  as a normal MCP tool.

## Example flow

1. An extension ingests a monthly sales report: extracts markdown, calls `put_doc` with tags
   `[sales, 2026-06]`, uploads the source XLSX as media, links it.
2. The doc-write reactor fires; the indexer chunks the new doc (say 14 chunks), finds all 14
   content hashes new, and calls the gateway `embed` under the workspace's budget.
3. 14 `doc_vector` rows upsert into the workspace's HNSW index (dim checked against the
   pinned index meta).
4. Later, the workspace agent handles "how did June sales compare to May?" — it calls
   `docs.search { query: "monthly sales results", filter: { tags: ["sales"] }, k: 8 }`.
5. The verb embeds the query (same pinned model), filters to `sales`-tagged docs the caller
   can reach, KNNs, re-gates each hit, and returns ranked chunks from the June and May docs.
6. The agent reads the full docs via `get_doc` and answers with `lb-doc://` citations.
7. Months later the workspace switches embedding models: an admin runs `docs.reindex` with
   the new model — a durable job re-embeds everything, updates the index meta, and reports
   per-doc results. Search is never half-migrated: the job swaps the index atomically at
   commit.

## Testing plan

Mandatory categories (per `scope/testing/testing-scope.md`):

- **Workspace isolation:** seed identical docs in two workspaces; search in one must never
  return, rank against, or count the other's vectors.
- **Capability deny:** caller without `docs.search` cap → denied; caller with search cap but
  without read reach on a matching doc → that hit absent from results (and proven absent,
  not error'd).
- **Offline/sync:** vectors don't sync; a doc synced to a second node becomes searchable
  there only after local indexing.

Key integration cases: dimension-mismatch rejection on a model swap without migration;
idempotent re-index (edit one section → only its chunks re-embed, verified via the mock's
call count); delete cascades; `docs.reindex` job resume mid-batch; deterministic
mock-provider ranking end-to-end.

## Risks & hard problems

- **Chunking quality is the product.** Bad chunking makes retrieval garbage regardless of
  model. Heading-aware + overlap is a defensible start, but expect iteration; keep the
  chunker a pure function with fixture tests so strategy changes are cheap re-index jobs.
- **Result gating cost.** k KNN hits × per-doc capability checks on every search; fine at
  k≤20, needs batched reach resolution if k grows.
- **Bulk-import budget burn.** A large corpus import triggers a large embed spend; the job
  must surface estimated/actual usage and respect gateway budget caps mid-run (truncate and
  report, not silently drop).
- **Freshness vs write latency.** The reactor path must never block or fail a doc write; a
  down gateway means stale index + a visible pending count, not write errors.
- **Model migration discipline.** The pinned model/dim meta record is the only thing standing
  between us and silently corrupted retrieval; every write path must check it.

## Open questions

- New `doc_vector` table (proposed) vs generalizing `tag_vector` into one `vector` table with
  a `kind` column — one index per kind either way; which is cleaner in SurrealDB?
- Chunking parameters: target tokens per chunk, overlap, and whether tables/code blocks are
  kept atomic. Fixture-driven decision during implementation.
- Is the doc-write reactor on by default, or opt-in per workspace until embed budgets are
  routinely configured?
- Hybrid scoring with SurrealDB FTS (BM25 + KNN fusion) — follow-up scope or fold in here if
  KNN-only retrieval proves weak on exact terms (part numbers, invoice ids)?
- Snippet shape: return chunk text verbatim, or windowed around the best match? Verbatim is
  simpler and the chunk is already read-gated.
- Does the query embedding cache by `(model, query-hash)` share the chunk idempotency cache,
  or is query traffic too unique to bother?

## Skill doc

Yes — this ships an agent-drivable surface. The implementing session writes and maintains
`docs/skills/doc-search/SKILL.md` (how to search docs, filter by tags, trigger/monitor a
reindex job), grounded in a live run.

## Related

- `../ai-gateway/ai-gateway-scope.md` — the contract this extends ("Embeddings, not just
  completions"; pin-the-model rule; budget/idempotency machinery reused as-is).
- `../document-store/document-store-scope.md` — the doc surface being indexed; link grammar
  for citations.
- `../document-store/doc-extraction-scope.md` — the upstream producer (file → doc); together
  they complete file → doc → vector → search.
- `../tags/tags-scope.md` — home of the existing HNSW primitive (`crates/tags/src/vector.rs`).
- `../jobs/jobs-scope.md` — `docs.reindex` is a durable batch job.
- `../agent/` — the primary consumer of `docs.search`.
- README `§6.1` (SurrealDB vector/HNSW), `§6.5`/`§6.6` (MCP + capability chokepoint),
  `§6.9` (jobs), `§6.14` (shared AI gateway).
