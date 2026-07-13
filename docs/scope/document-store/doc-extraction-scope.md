# Document-store scope — extraction (derive markdown docs from binary files)

Status: scope (the ask). Promotes to `public/document-store/` once shipped.

> Read with: `document-store-scope.md` (the markdown store + link graph this feeds),
> `../files/media-scope.md` (the binary path extraction reads from),
> `../embeddings/embeddings-scope.md` (the consumer — it deliberately "starts at a markdown
> doc exists"; this scope is what makes that true for real-world files), `../jobs/jobs-scope.md`
> (extraction runs as a job), README §3 rule 10 (generic seams, no product nouns).

The platform can store a PDF (media) and store markdown (docs), but nothing connects them:
there is **no path from a binary file to a searchable markdown document**. Every product
host that ingests real-world files — reports, spreadsheets, scans, exports — would today
pick its own parsing crates and re-invent provenance, idempotency, and re-run semantics,
N slightly-different ways. We want **one generic extraction seam in core**: given a media
record, derive a markdown **doc** (body + a `derived_from` edge back to the original),
through a per-mime extractor registry behind one trait, run as a durable job. The original
stays the source of truth; the derived doc is what the link graph, the agent, and the
embeddings pipeline consume. Extraction knows *mime types*, never domains.

## Goals

- **`Extractor` trait, per-mime registry:** `extract(bytes, mime, opts) -> Vec<ExtractedDoc
  { title_hint, markdown, part }>` — pure, deterministic, no network. v1 extractors:
  PDF (text-layer), XLSX/CSV (sheets → markdown tables with a size cap), HTML → markdown,
  plain text/markdown passthrough. One extractor per mime family, one file each
  (FILE-LAYOUT); unknown mime = honest `Unsupported`, never a silent empty doc.
- **`docs.extract` as a job:** input = media id(s), output = doc id(s); per-item results
  (extracted | unsupported | failed(reason)); multi-part sources (a workbook's sheets) may
  produce several docs, each edge-linked to the one original.
- **Provenance + idempotency:** an `extraction` record per derivation — `{ media_id,
  media_checksum, extractor_id, extractor_version, doc_ids, ts }`. Re-running with the same
  `(checksum, extractor version)` is a no-op; a bumped extractor version re-derives and
  **updates the same docs** (stable derived-doc identity, so links and embeddings migrate
  instead of orphaning).
- **The derivation edge:** derived doc → source media as a first-class relation (the
  document-store's edge machinery), so "show me the original" is one hop, and deleting the
  media flags — not silently breaks — its derived docs.
- **Caller-supplied doc fields:** title override, tags, visibility ride the job request and
  land on the derived docs; core never invents domain metadata (rule 10 — tags like
  `sales` are the caller's business, literally).

## Non-goals (v1)

- **OCR / scanned documents** — a later extractor behind the same trait (it drags a heavy
  dependency and non-determinism; the seam is shaped for it, v1 refuses image-only PDFs
  honestly as `Unsupported`).
- **Model-assisted extraction** ("ask an LLM to transcribe the chart") — a later extractor
  that routes through the ai-gateway under the caller's budget; the trait's `opts` leaves
  room. v1 is pure parsing only, so extraction works offline and costs nothing.
- **Embedding/indexing** — the embeddings scope owns doc→vector; extraction just produces
  the docs its reactor picks up.
- **DOCX/PPTX and email bodies** — follow-up extractors (email arrives via
  `../inbox-outbox/mail-source-scope.md`, which stores the raw message as media and calls
  this seam like any other caller).
- **Editing derived docs.** A derived doc is regenerable output; hand-edits would be lost
  on re-extraction. v1 marks derived docs read-only-by-convention (flagged in Risks; a
  fork-to-editable-copy verb is the escape hatch).

## Intent / approach

A host service (`crates/host/src/assets/extract/` beside the doc verbs) + a small pure
crate (`crates/extract`) holding the trait and the v1 extractors, so the parsing logic has
zero host dependencies and is fixture-testable in isolation.

*Rejected: extraction inside each product extension* — N hosts × parsing crates ×
provenance schemes, and the embeddings pipeline can never assume derived docs exist.
*Rejected: extraction inside `media.upload_commit`* — couples uploads to parsing cost and
makes "store a photo" pay a document tax; extraction is an explicit, capability-gated act.
*Considered: a Tier-2 sidecar for isolation* (parsing untrusted binaries in-process is the
real objection) — v1 keeps extractors in-process but pure and panic-contained per item
(a panicking extractor fails that item, not the job); promoting the registry behind the
existing native-sidecar seam is a named follow-up if the threat model hardens (see Risks).

## How it fits the core

- **Tenancy / isolation:** the job reads media and writes docs inside one workspace; the
  extraction record lives beside them. Cross-workspace derivation is impossible by
  construction (all ids resolve inside the caller's namespace).
- **Capabilities:** `mcp:docs.extract:call` + read reach on the source media + doc write —
  the job runs under the **caller's** principal, never a widened service identity. Deny
  path: media the caller can't read → per-item `denied`, not a leak via the derived doc.
- **Placement:** either. Pure parsing, no network — an offline edge node extracts identically.
- **MCP surface (§6.1):** one **batch job** verb (`docs.extract`; even one large PDF can run
  long — no synchronous variant, callers needing "small and now" pass one id and watch the
  job). No CRUD (derived docs are written through the existing doc verbs), no live feed
  (job status is the progress surface).
- **Data (SurrealDB):** `extraction` records + `derived_from` relation edges; derived docs
  are ordinary `doc:{id}` rows. State only.
- **Bus (Zenoh):** none new — job progress is the platform's; the doc-write events the
  derived docs emit are what the embeddings reactor already listens to.
- **Sync / authority:** extraction records and edges sync like any state; a node can also
  always re-derive locally from synced media (derived data posture, same as vectors).
- **Secrets:** none.
- **No mocks:** fixture binaries (a real PDF, a real workbook) live in the repo; extractor
  output is snapshot-tested. No fake store; the one permitted fake is nothing — extraction
  has no external.
- **SDK/WIT impact:** none — extensions call `docs.extract` over the normal host-callback
  MCP surface.

## Example flow

1. A product extension ingests a quarterly report: resumable-uploads the PDF as media,
   then calls `docs.extract { media: [id], tags: [...], visibility }`.
2. The job checks the extraction ledger — new `(checksum, extractor@version)` — parses the
   text layer, writes one markdown doc, links it `derived_from` the media, records the
   extraction.
3. The doc-write reactor (embeddings scope) picks the new doc up; it becomes searchable.
4. A workbook goes through the same verb and yields one doc per sheet (caller opted
   `split: per_part`), all edged to the one original.
5. Months later the PDF extractor improves (v2 handles multi-column). `docs.extract` with
   `force_version: 2` over the corpus re-derives in place: same doc ids, updated bodies —
   backlinks survive, and the embeddings reactor re-embeds only the changed chunks.

## Testing plan

Mandatory categories (`../testing/testing-scope.md`):

- **Workspace isolation:** media in ws A named from ws B → per-item not-found; derived
  docs never cross.
- **Capability deny:** caller without `docs.extract` → denied; caller without read reach on
  one of three media ids → that item `denied`, the other two extracted.
- **Offline:** extraction succeeds with no network (pure parsers).

Key cases: snapshot tests per fixture per extractor (the fidelity contract); idempotent
re-run (ledger hit → no-op, verified by extraction-record count); extractor version bump
re-derives into the **same** doc ids; multi-sheet split; corrupt/truncated file → per-item
`failed`, job completes; panicking extractor contained to its item; unsupported mime →
`Unsupported`, never an empty doc.

## Risks & hard problems

- **Fidelity is the product.** Text-layer PDF extraction loses tables and layout at the
  worst moments; snapshot fixtures make regressions visible, but callers must treat derived
  docs as lossy and keep the original one edge away (the design already forces this).
- **Untrusted input in-process.** PDF parsers are a classic attack surface. v1 mitigations:
  pure-Rust parsers only, per-item panic containment, size limits from media config. The
  sidecar promotion is the real fix if exposure grows — named follow-up, not hand-waved.
- **Derived-doc identity vs hand edits.** Users *will* edit a derived doc; re-extraction
  then clobbers. v1's read-only convention + fork verb is a soft answer; a `rev`-conflict
  guard (refuse to overwrite a doc whose rev moved since derivation) is the honest backstop.
- **Extractor version churn** mirrors embedding-model churn — both are "derived data
  migrations." Keeping the version in the ledger makes re-derivation explicit and auditable;
  silent extractor upgrades are forbidden.

## Skill doc

Yes — `docs/skills/doc-extraction/SKILL.md`: how to extract a file into docs, split
options, re-running after an extractor upgrade, reading per-item job results. Written by
the implementing session from a live run.

## Open questions

- Crate choices: `pdf-extract`/`lopdf` (pure Rust) vs `pdfium` bindings (better fidelity,
  heavier/unsafe) — fixture quality decides; `calamine` for XLSX seems settled.
- Default split policy for multi-part sources: one doc per workbook with sheet headings, or
  per-sheet docs? (Caller-overridable either way; pick the default from real fixtures.)
- Where does the derived-doc read-only convention live — a `derived` flag on the doc record
  the UI/verbs respect, or convention only in v1?
- Size caps for embedded tables (a 10k-row sheet should summarize + link, not inline) —
  fixed cap or caller opt?
- Does `docs.extract` accept binary-asset ids too, or media only? (Document-store's
  attachments vs the media path — probably both, one resolver.)

## Related

- `document-store-scope.md` — the store and edges this writes into.
- `../files/media-scope.md` — the upload path this reads from.
- `../embeddings/embeddings-scope.md` — the downstream consumer; together they complete
  file → doc → vector → search.
- `../inbox-outbox/mail-source-scope.md` — a producer that calls this seam.
- `../jobs/jobs-scope.md`; README §6.1, §6.5/§6.6, §6.12, §3 rule 10.
