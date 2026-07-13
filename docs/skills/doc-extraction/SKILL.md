---
name: doc-extraction
description: >-
  Derive markdown docs from binary media (PDF/XLSX/CSV/HTML/text) over the `docs.extract` MCP verb.
  Use when a task says "extract this PDF into a doc", "turn the uploaded spreadsheet into markdown",
  "ingest a file into the document store", "re-derive docs after the extractor improved", "why did
  extraction return unsupported/failed/denied", or "how do the per-item extraction results work".
  Covers the single-verb batch job, its per-item outcomes (extracted | unsupported | failed |
  denied), the idempotency ledger, version-bump re-derivation into stable doc ids, split policy for
  multi-sheet workbooks, and the capability model. Grounded in a live node run.
---

# Extract markdown docs from binary media (`docs.extract`)

`docs.extract` turns a stored **media** record (a PDF, workbook, CSV, HTML, or text file) into a
markdown **doc**, links the doc back to the source with a `derived_from` edge, and records the
derivation in a provenance ledger so re-runs are idempotent. The original media stays the source of
truth; the derived doc is what the link graph, the agent, and the embeddings pipeline consume.

It knows **mime types, never domains** (rule 10). You supply `title`, `tags`, and split policy;
extraction never invents metadata.

- **One verb:** `docs.extract` — a batch job over media id(s), gated `mcp:docs.extract:call`.
- **Input** is media that already exists (upload it via the media path first). **Output** is a job
  id + one result per media id.
- **Pure + offline:** v1 parses text layers only (no OCR, no model calls) — extraction costs nothing
  and works on an offline edge node.

## Prerequisites

- A session token (`POST /login`) whose caps include `mcp:docs.extract:call`, plus read reach on the
  source media (`mcp:media.get:call`) and doc write (`store:doc/*:write`). The built-in member/author
  role bundle carries all three.
- The source bytes stored as **media** (the chunked-upload path: `media.upload_begin` →
  `PUT /media/{id}/chunk/{n}` → `media.upload_commit`). Extraction reads `media_get` + the chunk
  bytes; it never uploads for you.

All calls below go to `POST /mcp/call` with `Authorization: Bearer <token>` and a body
`{ "tool": "<verb>", "args": { … } }`. The transcript below is a real run against a dev node
(`user:ada` @ `acme`).

## 1. Extract one file

```jsonc
// POST /mcp/call
{ "tool": "docs.extract", "args": {
    "media": "report-q3",         // a single media id (string) OR an array of ids
    "tags": ["reports", "2026-q3"],
    "ts": 300
} }
```

```jsonc
// → 200
{
  "job_id": "docs-extract-user:ada-300",
  "items": [
    { "status": "extracted", "media_id": "report-q3",
      "doc_ids": ["derived_from-report-q3:pdf-text"], "reused": false }
  ]
}
```

Read the derived doc with the normal doc verb:

```jsonc
// { "tool": "assets.get_doc", "args": { "id": "derived_from-report-q3:pdf-text" } }
// →
{ "id": "derived_from-report-q3:pdf-text", "content_type": "markdown",
  "title": "Quarterly Report", "tags": ["reports","2026-q3"],
  "content": "Quarterly Report\nPage one covers revenue.\nTotal was up ten percent.\n\nAppendix\n…" }
```

The derived doc id is **stable and deterministic**: `derived_from-{media_id}:{extractor_id}` (plus
`:{part}` per sheet under `per_part`). You do not choose it — that stability is what makes re-runs
and re-derivation land on the same doc.

## 2. Per-item results — the four outcomes

`docs.extract` is a batch: pass several media ids, get one result each. The **job completes even
when individual items fail**.

```jsonc
// { "tool": "docs.extract", "args": { "media": ["report-q3", "no-such-media"], "ts": 600 } }
// →
{ "job_id": "docs-extract-user:ada-600", "items": [
    { "status": "extracted", "media_id": "report-q3", "doc_ids": ["derived_from-report-q3:pdf-text"], "reused": true },
    { "status": "denied",    "media_id": "no-such-media" }
] }
```

| `status` | Meaning | Fields |
|---|---|---|
| `extracted` | Docs derived (or a ledger no-op — see §3) | `doc_ids`, `reused` |
| `unsupported` | No extractor for the mime, or a v1 non-goal (image-only PDF) — **never an empty doc** | `reason` |
| `failed` | The bytes were corrupt/truncated/malformed (or the parser panicked, contained to this item) | `reason` |
| `denied` | You can't read that media (missing reach, another workspace's id, or it doesn't exist — indistinguishable, no existence leak) | — |

An unsupported mime is honest, not silent:

```jsonc
// a media whose mime is application/zip →
{ "status": "unsupported", "media_id": "mystery-blob", "reason": "no extractor for mime application/zip" }
```

Supported mimes: `application/pdf` (text layer), the XLSX mime
`application/vnd.openxmlformats-officedocument.spreadsheetml.sheet`, `text/csv`, `text/html` /
`application/xhtml+xml`, and `text/plain` / `text/markdown`.

## 3. Idempotent re-runs (the ledger)

Re-extracting the same media at the same extractor version is a **no-op** — the extraction ledger is
hit and the existing doc ids come back with `reused: true`. Safe to call on every upload without
re-deriving or duplicating.

```jsonc
// second call, same media, same version →
{ "status": "extracted", "media_id": "report-q3",
  "doc_ids": ["derived_from-report-q3:pdf-text"], "reused": true }
```

## 4. Re-derive after an extractor upgrade (`force_version`)

When an extractor improves, re-derive a corpus in place with `force_version` set to the new floor.
The derived doc ids are stable, so re-derivation **updates the same docs** — backlinks and embeddings
migrate instead of orphaning. Silent extractor upgrades are forbidden; a version bump is explicit.

```jsonc
// { "tool": "docs.extract", "args": { "media": "report-q3", "force_version": 2, "ts": 500 } }
// →
{ "status": "extracted", "media_id": "report-q3",
  "doc_ids": ["derived_from-report-q3:pdf-text"], "reused": false }   // re-derived, SAME id
```

> Re-derivation overwrites the derived doc from the source — including its `title`/`tags`. Re-supply
> the caller fields you want on the re-derived doc (they are yours to set each run). A derived doc is
> regenerable output; treat it read-only (hand edits are lost on the next extraction).

## 5. Multi-sheet workbooks — split policy

A workbook is multi-part. `split` (default `"whole"`) controls the shape:

- `"whole"` — one doc, each sheet a `## {sheet}` section (best for reading; keeps search/backlinks
  together).
- `"per_part"` — one doc per sheet, each keyed by the sheet name → its own stable derived doc id.

```jsonc
// { "tool": "docs.extract", "args": { "media": "sales-book", "split": "per_part", "ts": 300 } }
// → items[0].doc_ids has one id per sheet:
//   ["derived_from-sales-book:xlsx:Sales", "derived_from-sales-book:xlsx:Regions"]
```

Large tables are truncated at a row boundary with an honest `_… N more rows elided (source has M
rows) …_` marker — the full data is always one `derived_from` edge away on the original.

## 6. HTML → markdown (best-effort)

HTML extraction is best-effort markdown (headings, lists, links, emphasis; scripts/styles dropped).
Internal `lb-doc://` / `lb-asset://` links in the source survive into the derived doc, so the
document-store link graph picks them up as edges. Real output from the live run:

```markdown
# Cooler Maintenance

Check the **compressor** and the *fan* monthly.

## Steps

- Power off the unit
- Inspect wiring & connectors
- See [the alarm matrix](lb-doc://alarm-matrix)
```

The raw HTML media is the fidelity escape hatch when best-effort isn't enough.

## Capability & isolation notes

- `docs.extract` runs under **your** principal — never a widened service identity. Each source media
  is re-checked for read reach **per item** (unreadable → that item `denied`, the rest still
  extract), and each derived doc is written through the normal `store:doc/*:write` gate.
- Everything is workspace-scoped: naming another workspace's media id yields `denied` (no existence
  leak) and derives nothing cross-workspace.
- Without `mcp:docs.extract:call` the whole call is denied (`403`, opaque) before any item runs.

## Gotchas

- **Upload media first.** `docs.extract` reads existing media; it does not accept raw bytes. Use the
  media upload path, then pass the returned media id.
- **`media` takes a string or an array.** A single id as a bare string is accepted (the "one file,
  now" convenience); multiple ids as an array run as one batch.
- **The `job_id` is for audit/idempotency.** v1 extraction is pure and in-process, so the per-item
  results are already in the response — you don't poll the job. (When OCR/model-assisted extractors
  add latency, the same verb moves to the detached job-watch pattern.)
- **`ts` is a caller-supplied logical timestamp** (determinism — no wall clock in the core). Any
  monotonic value is fine.
