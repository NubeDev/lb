# Document store

The document store holds markdown **docs** and a link graph over them, plus the binary
**media** those docs can be derived from. This page covers **extraction** — the path from a
binary file to a searchable markdown doc. (The store + link-graph write path is documented
alongside the `assets.*` verbs.)

## Extraction — binary media → markdown docs

The platform can store a PDF (as media) and store markdown (as a doc), and **`docs.extract`
connects them**: given a media record, it derives a markdown doc, links the doc back to the
original with a `derived_from` edge, and records the derivation in a provenance ledger. The
original stays the source of truth; the derived doc is what the link graph, the agent, and
the embeddings pipeline consume.

Extraction knows **mime types, never domains**. It never invents metadata — the caller
supplies `title`, `tags`, and split policy, and those ride onto the derived docs verbatim.

### The pure extractor layer (`lb-extract`)

Parsing lives in a host-free crate (`crates/extract`) behind one trait, so it is
deterministic and offline (no network, no clock). One extractor per mime family:

| Mime | Extractor | Output |
|---|---|---|
| `application/pdf` | `pdf-text` | text-layer → markdown (image-only/scanned PDFs → honest `unsupported`; OCR is a follow-up) |
| `…spreadsheetml.sheet` (XLSX) | `xlsx` | sheets → markdown tables (one doc, or one per sheet) |
| `text/csv` | `csv` | rows → a markdown table |
| `text/html`, `application/xhtml+xml` | `html` | best-effort markdown (headings, lists, links, emphasis) |
| `text/plain`, `text/markdown` | `text` | passthrough (UTF-8 validated) |

An **unknown mime has no extractor** — the item comes back `unsupported`, never a silent
empty doc. Tables past a cell cap are truncated at a row boundary with an honest
`_… N more rows elided …_` marker (the original is one edge away for the full data).

### The `docs.extract` verb

One batch job verb. Input is media id(s) + optional doc fields; output is a job id plus a
per-item result for each media id:

```jsonc
// request
{ "tool": "docs.extract", "args": {
    "media": ["report-q3", "sales-book"],   // one id or an array
    "tags": ["reports", "2026-q3"],          // caller's business (rule 10)
    "title": "Q3 report",                    // optional override
    "split": "per_part"                       // "whole" (default) | "per_part" (workbook → per-sheet docs)
} }

// response
{ "job_id": "docs-extract-user:ada-…", "items": [
    { "status": "extracted",  "media_id": "report-q3", "doc_ids": ["derived_from-report-q3:pdf-text"], "reused": false },
    { "status": "unsupported", "media_id": "sales-book", "reason": "no extractor for mime …" }
] }
```

Per-item statuses: **`extracted`** (with the derived `doc_ids`; `reused: true` on an
idempotent re-run), **`unsupported`** (no extractor / v1 non-goal), **`failed`** (corrupt
input, with a reason — the job still completes), **`denied`** (the caller can't read that
media — no existence leak; the other items still extract).

### Provenance, idempotency, and re-derivation

Each derivation writes an **extraction ledger** record
(`{media_id, media_checksum, extractor_id, extractor_version, doc_ids, ts}`):

- **Idempotent re-run.** Re-extracting the same media at the same extractor version is a
  no-op — the ledger is hit and the existing `doc_ids` are returned (`reused: true`).
- **Stable derived-doc identity.** A changed source (new checksum) or a bumped extractor
  version (`force_version`) re-derives into the **same doc ids** — so backlinks and
  embeddings migrate in place instead of orphaning. Silent extractor upgrades are
  forbidden; a version bump is explicit and auditable in the ledger.

### Security & isolation

`docs.extract` is gated by `mcp:docs.extract:call`, and the job runs under the **caller's**
principal — never a widened service identity. Each source media is re-gated for read reach
per item (media the caller can't read → that item `denied`), and each derived doc is written
through the normal doc-write gate (`store:doc/*:write`). Everything is workspace-scoped:
cross-workspace derivation is impossible by construction (all ids resolve inside the caller's
namespace). Extraction is pure and in-process; a panicking parser is contained to its item,
never the job.

### Limits (v1)

Text-layer parsing only — no OCR, no model-assisted transcription, no DOCX/PPTX (all are
follow-up extractors behind the same trait). HTML→markdown is best-effort; the raw media is
always one `derived_from` edge away as the fidelity escape hatch. Derived docs are
regenerable output — treat them read-only (a re-extraction overwrites hand edits).

See the runnable operating guide: `docs/skills/doc-extraction/SKILL.md`.
