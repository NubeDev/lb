# Document-store — doc extraction (session)

- Date: 2026-07-13
- Scope: ../../scope/document-store/doc-extraction-scope.md
- Stage: S8+ (data plane) — building on the shipped doc/media surfaces
- Status: done

## Goal
Implement `doc-extraction-scope.md` end to end: a pure `crates/extract` (per-mime
`Extractor` trait + PDF/XLSX/CSV/HTML/text extractors, no host deps, offline,
fixture-tested) and a host service under `crates/host/src/assets/extract/` — the
capability chokepoint, the `docs.extract` job, the extraction ledger (provenance +
idempotency), and the `derived_from` edges — plus the `docs.*` MCP bridge. Generic
(mime types, never domain nouns; rule 10). Downstream embeddings scope must stay
seam-compatible ("a markdown doc exists").

## What changed
(to fill as I build — link files as path:line)

### Architecture map (from reading the siblings)
- **Input is `media`** (`crates/host/src/media/`) — chunked binary, `read_all_bytes`,
  a `checksum` field, `mime`. Not `asset` (inline blob). Scope §"reads from" =
  media-scope. Open q "media only or assets too" → resolve to media in v1 (one
  resolver; asset ids are a follow-up — noted below).
- **Output is `doc`** written through `lb_assets::put_doc` with `ContentType::Markdown`
  (so the existing link-graph edge extraction runs) + `derived_from` relation edge via
  `lb_assets::relate`.
- **Capability chokepoint** mirrors `assets/authorize.rs`: workspace-first, then cap.
  New verb `docs.extract` gated `mcp:docs.extract:call`; per-item source-media read
  reach re-checked under the caller's principal (media read = `media.get` cap today —
  see decision below).
- **Job pattern** = `devkit/build.rs`: `Job::new(id, kind, payload, ts)` → `create`
  → work → `complete(status)`, returns `{job_id}` immediately. Per-item extraction
  results are durable in the **ledger** (one `extraction` record per media item), so
  the transcript-shaped `lb_jobs::Step` (agent-only) is not abused for item rows.
- **MCP routing touch-points** for a NEW host-native prefix (`tool_call.rs`):
  1. `HOST_NATIVE_PREFIXES` += `"docs."`
  2. a dispatch arm `qualified_tool.starts_with("docs.")` → `call_docs_tool`
  3. `system/catalog.rs` picks prefixes up from the shared const (no manual mirror)
  4. `lib.rs` re-export of `call_docs_tool`
  (`gate_tool_for` alias only if a verb rides another verb's cap — extract is its own.)

## Decisions & alternatives
- **`docs.` is a NEW native prefix**, not a new arm in `call_asset_tool`. Doc verbs are
  today exposed as `assets.put_doc`/`assets.get_doc` (the `assets.` prefix). The scope
  names the verb `docs.extract` verbatim and the embeddings scope names `docs.search` /
  `docs.reindex` — so `docs.` is the shared namespace for doc-derived operations. New
  prefix keeps extract/search/reindex together and out of the asset CRUD bridge.
- **Ledger records ARE the per-item results.** `lb_jobs` steps are agent-transcript
  typed events; forcing extraction item outcomes through them would abuse that vocab.
  The `extraction` record (`{media_id, media_checksum, extractor_id, extractor_version,
  doc_ids, status, ts}`) is the durable per-item truth; job terminal status = "batch ran".
- **Pure crate holds zero host types.** `Extractor::extract(bytes, mime, opts) ->
  Result<Vec<ExtractedDoc>, ExtractError>`; `ExtractedDoc { title_hint, markdown, part }`.
  Registry is a pure `fn extractor_for(mime) -> Option<&dyn Extractor>` so the host maps
  `None → Unsupported`.
### Resolved from real fixture output
- **Crate choices** (open q1): `pdf-extract` 0.7 (pure Rust, text-layer) — extracted the
  2-page fixture cleanly; `calamine` 0.26 (pure Rust XLSX, no Excel/C dep); `csv` 1
  (already in workspace). HTML → a **dependency-light in-crate scanner** (no html2text/
  scraper): HTML fidelity is best-effort per scope, and a focused tag walk over headings/
  paragraphs/lists/links/emphasis is the right machinery; the raw media is the fidelity
  escape hatch. No `pdfium`/unsafe bindings in v1 (scope rejected: heavier/unsafe).
- **Default split policy** (open q2): **`Whole`** — the multi-sheet fixture reads best as
  ONE doc with `## {sheet}` sections; per-sheet docs fragment search/backlinks. `PerPart`
  is caller-overridable (stable `part` key = sheet name → stable derived doc id).
- **Table size cap** (open q4): a `max_table_cells` opt (default 5000); past it the table
  truncates at a row boundary with an honest `_… N more rows elided …_` marker naming the
  source total. Proven by `xlsx_table_cell_cap_truncates_with_marker`.
- **Determinism**: `pdf-extract` can panic on malformed PDFs — the crate ALSO `catch_unwind`s
  and converts to `Failed` (defense in depth; the host contains per-item panics too). Image-
  only PDF (empty text layer) → honest `Unsupported`, never an empty doc.

## What changed (concrete)
- **New pure crate `crates/extract`** (`lb-extract`): `Extractor` trait
  ([trait_def.rs](../../../rust/crates/extract/src/trait_def.rs)), value types
  ([model.rs](../../../rust/crates/extract/src/model.rs)), `Unsupported`/`Failed` error
  ([error.rs](../../../rust/crates/extract/src/error.rs)), mime→extractor registry
  ([registry.rs](../../../rust/crates/extract/src/registry.rs)), and five extractors
  (`extractor/{pdf,xlsx,csv,html,text}.rs` + shared `table.rs`). Zero host deps.
- **Host service `crates/host/src/assets/extract/`**: the `docs.extract` orchestrator
  ([extract.rs](../../../rust/crates/host/src/assets/extract/extract.rs)), per-item derivation
  with panic containment ([derive.rs](../../../rust/crates/host/src/assets/extract/derive.rs)),
  the capability chokepoint ([authorize.rs](../../../rust/crates/host/src/assets/extract/authorize.rs)),
  the idempotency ledger ([ledger.rs](../../../rust/crates/host/src/assets/extract/ledger.rs) +
  [model.rs](../../../rust/crates/host/src/assets/extract/model.rs)), the `derived_from` edge, and
  the `docs.*` MCP bridge + descriptor ([tool.rs](../../../rust/crates/host/src/assets/extract/tool.rs)).
- **Wiring (the "new native prefix" = 5 touches):** `HOST_NATIVE_PREFIXES += "docs."` + dispatch arm
  in [tool_call.rs](../../../rust/crates/host/src/tool_call.rs); catalog row in
  [system/catalog.rs](../../../rust/crates/host/src/system/catalog.rs) (satisfies the
  `host_catalog_covers_dispatch_prefixes` tripwire); cap `mcp:docs.extract:call` in
  [builtin_roles.rs](../../../rust/crates/host/src/authz/builtin_roles.rs); descriptor registered in
  [tools/descriptor.rs](../../../rust/crates/host/src/tools/descriptor.rs); re-exports in
  `assets/mod.rs` + `lib.rs`. Made `media::model` `pub(crate)` so extraction reads media bytes.

## Tests
Mandatory categories covered: **capability-deny** (`denies_extract_without_cap`,
`per_item_denied_when_media_unreadable`), **workspace-isolation**
(`media_in_other_workspace_is_per_item_denied`), **offline** (the entire `lb-extract` suite is pure/
no-network by construction — no fake anywhere, per testing §0). Plus every key case from the scope:
idempotent re-run, version-bump re-derive into the same doc ids, per-item corrupt-fail while the job
completes, unsupported mime, panic containment.

```
$ cargo test -p lb-extract
  errors.rs:   test result: ok. 7 passed; 0 failed
  snapshot.rs: test result: ok. 6 passed; 0 failed     (the fidelity contract, 6 committed .snap goldens)

$ cargo test -p lb-host --test doc_extraction_test
  test result: ok. 10 passed; 0 failed

$ cargo test -p lb-host --lib  (blast-radius unit tests)
  system::catalog / tools::descriptor / assets::extract::derive::panic_is_contained_as_failed
  test result: ok. 9 passed; 0 failed

# No new regressions — touched-surface integration binaries all green:
  authz_test 7 | assets_doc_test 6 | assets_mcp_test 4 | assets_isolation_test 3 | catalog_mcp_test 8
  media_test 20 | document_store_test 9 | system_map_test 13 | authz_mcp_dispatch_test 5
```
`make build-wasm` green; `cargo build --workspace` green. (Did NOT run the full `cargo test
--workspace` to completion — it is fail-fast with ~7 known pre-existing red binaries and OOMs at high
test-thread counts on this box; instead ran the full blast radius binary-by-binary, all green above.
See [[preexisting-failing-tests]].)

### Live run (SKILL grounding)
Booted `./target/debug/node` on `127.0.0.1:8099` (in-mem, `LB_DEV_LOGIN=1`, seed `user:ada`@`acme`),
seeded the committed PDF/HTML fixtures as media via `store.write`, and drove `docs.extract` over
`POST /mcp/call`. Verified live: first-run `extracted`, re-run `reused:true`, `force_version:2`
re-derive into the same doc id, `assets.get_doc` returning the real markdown, unsupported-mime and
denied-item outcomes, and the single-id **string** `media` form. Transcripts pasted into the SKILL.

## Debugging
None opened (no bug against a shipped surface — the one issue was in this session's own new code:
the `docs.extract` descriptor declared `media` as strictly `type:array`, which the defense-in-depth
`validate_args` used to 403 the single-id string form the handler supports. Fixed in the same session
by un-`type`ing the `media` property in the descriptor (handler stays authoritative); re-verified
live. Not a regression → no `debugging/` entry, noted here.)

## Public / scope updates
- Filled the extraction section of
  [doc-site/content/public/document-store/document-store.md](../../../doc-site/content/public/document-store/document-store.md).
- Resolved all five open questions in
  [doc-extraction-scope.md](../../scope/document-store/doc-extraction-scope.md) (crate choices,
  default split = `whole`, table cap = opt+default, media-only in v1, read-only convention).
- Filed a new follow-up stub
  [derived-doc-rev-guard-scope.md](../../scope/document-store/derived-doc-rev-guard-scope.md) for the
  re-extraction-vs-hand-edits `rev` guard + `docs.fork` (named in the scope's Risks; NOT worked around).

## Skill docs
Wrote [docs/skills/doc-extraction/SKILL.md](../../skills/doc-extraction/SKILL.md) — the drivable
`docs.extract` surface, grounded in the live run above (real request/response pairs pasted, not
remembered).

## Dead ends / surprises
- **Dev-login can't drive `media.upload_begin` over MCP.** The dev token carries verb-suffix cap
  wildcards (`mcp:*.get/create/list/…`) + specific `mcp:media.upload:call`, but the OUTER dispatch
  gate needs `mcp:media.upload_begin:call` (the full tool name) — no `gate_tool_for` alias maps
  `media.upload_begin → media.upload`. So for the live run I seeded media via `store.write` (which
  the `mcp:*.write:call` wildcard + `store:*:write` DO cover). Not my surface to fix; noted as a
  possible pre-existing gap for the media/auth owners.
- **`lb_jobs` is agent-transcript-shaped**, not a generic per-item batch primitive — confirmed the
  right call is "durable job record for audit + the ledger IS the per-item truth", not bending
  `Step`/`TranscriptEvent` to hold extraction outcomes.
- `store.query SELECT * FROM extraction` errors on the ledger's serde-tagged enum (a store.query JSON
  quirk) — the ledger is proven via the typed `get_extraction` in tests instead.

## Follow-ups
- `derived-doc-rev-guard-scope.md` (filed) — the honest backstop for hand-edited derived docs.
- Embeddings scope stays seam-compatible: it "starts at a markdown doc exists", and `docs.extract`
  produces exactly that (a `ContentType::Markdown` doc emitting the doc-write event its reactor
  listens to). **No interface I built forces a change to the embeddings or mail-source scope docs** —
  did not touch either implementation, as instructed.
- Possible media/auth gap: `media.upload_begin/commit` unreachable by a token holding only
  `mcp:media.upload:call` (see Dead ends) — for the media owners to confirm/alias.
- STATUS.md: not updated this session (extraction is a data-plane slice under S8; leaving the
  dashboard edit to the stage owner unless asked).
