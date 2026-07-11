# Files scope — media (photo-class binaries: mobile upload, variants, streaming serve)

Status: scope (the ask). Promotes to `public/files/` once shipped.

> Read with: `files-scope.md` (files as shared workspace assets — the substrate),
> `../document-store/document-store-scope.md` (which already lands §6.12 images/attachments
> for *documents*; this scope generalizes the binary path for **high-volume, mobile-origin
> media**), `../extensions/extensions-scope.md` (host-callback ABI), README §3 rule 2
> (one datastore — SurrealDB, no separate blob service).

The platform can attach a file to a document, but it has no answer for **media at product
volume**: a phone uploading a 6 MB photo over flaky cellular for every child every day, a
feed thumbnail that must not pull the original, a video clip that needs range requests. The
strain is already visible — the ext-publish 413 body-limit fix raised one route's ceiling by
hand, and every byte currently rides inside a tool-call payload. We want a **first-class
media path** on the one datastore: chunked/resumable upload, server-side **variants**
(thumbnail/preview), and a capability-checked streaming serve route — generic (a "media" is
opaque bytes + mime; childcare photos, site images, and doc attachments are all callers).

## Goals

- **`media` record + buckets:** metadata row (`media:{ws}:{id}`: mime, bytes, checksum,
  owner, `origin` opaque ref, variant list) with content in SurrealDB buckets, chunked at a
  fixed size — one datastore, no blob sidecar (rule 2 holds).
- **Resumable upload:** `media.upload_begin` (declares size/mime/checksum → upload id) →
  `PUT /media/{id}/chunk/{n}` (idempotent, any order, retry-safe) → `media.upload_commit`
  (verifies checksum, flips `ready`). A dropped connection resumes, never restarts.
- **Variants as a job:** on commit, a durable **job** derives configured variants
  (`thumb`/`preview` for images v1); the record lists them; the feed asks for
  `?variant=thumb` and never ships an original by accident.
- **Serve route:** `GET /media/{id}` — workspace + capability + (when
  `entity-scoped-grants` lands) scope-checked, ETag/if-none-match, Range support, correct
  mime. Cacheable *after* the auth gate.
- **Limits as config:** per-mime max size, per-workspace quota, allowed-mime allowlist —
  `BootConfig`/prefs, not hardcoded (the 413 lesson: limits must be one governed knob).
- **SDK surface:** extensions reference media by id (`media.attach`-style opaque refs) and
  request upload tickets for their callers via host callback — an extension never
  proxies bytes through its own tool payloads.

## Non-goals

- **No video transcoding** — store + range-serve video v1; transcode is a later job type.
- **No CDN/edge-cache tier** — design leaves room (immutable variant URLs) but ships none.
- **No public/unauthenticated serving** — same deferral (and threat model) as the
  document-store's public slice.
- **Not replacing document attachments** — document-store keeps its surface; this is the
  shared binary path underneath both (one implementation, two callers).

## Intent / approach

The upload is the hard part and it is a **protocol**, not a bigger body limit: begin/chunk/
commit with idempotent chunks is what survives cellular. Variants are exactly what the jobs
substrate exists for (long, retryable, resumable — never inline in the commit call).

**Rejected — raise body limits and POST whole files:** already proven brittle (the 413
history); no resume, no progress, holds a gateway worker for the whole transfer.
**Rejected — filesystem/S3 blob store:** violates rule 2 (one datastore), splits
backup/isolation/sync stories in two. If bucket throughput becomes a real ceiling, that is
a measured SurrealDB problem to fix, not an architecture fork.

## How it fits the core

- **Tenancy / isolation:** media rows + buckets keyed by workspace; the serve route checks
  workspace-first like every read.
- **Capabilities:** `mcp:media.upload/get/delete:call` + the serve route enforcing read
  caps; deny = 403 before any byte. Scoped grants narrow reads per-record when available.
- **Placement:** either role. Uploads land on the node that serves the caller; sync of
  media across nodes rides the existing sync scope (flagged in open questions).
- **MCP surface (§6.1):** CRUD (`upload_begin/commit`, `delete` → archive), get/list
  (metadata; bytes go over HTTP, not MCP payloads), live feed N/A, batch = the variant
  **job** (and bulk import later).
- **State vs motion:** bytes + metadata are state; "media ready" is a small bus event for
  feed UIs; the variant job is jobs-substrate.
- **SDK/WIT impact — flagged:** additive host callbacks (`media.ticket`, `media.stat`).
  Additive WIT only.

## Example flow

1. Staff phone: extension asks `media.ticket` → `upload_begin{mime: image/jpeg, bytes:
   5_800_000, checksum}` → upload id + chunk size.
2. 6 chunks PUT; chunk 4 times out and is retried (idempotent).
3. `upload_commit` verifies the checksum → `ready`; the thumbnail job runs; `media.ready`
   event fires.
4. The extension writes its domain record holding `media_id`; feed clients render
   `GET /media/{id}?variant=thumb` (ETag-cached); tapping fetches the original with Range.
5. A guardian without reach on that record: `GET /media/{id}` → 403, zero bytes.

## Testing plan

Mandatory: **capability-deny** (serve + upload without caps → 403), **workspace isolation**
(id from ws A unreachable with a ws-B token). Plus: resume after dropped chunk, commit with
bad checksum → rejected + no partial record, duplicate chunk idempotent, oversize rejected
at `begin` (not after transfer), variant job failure → original still `ready` + job retries,
Range requests, ETag 304s. Real store, real gateway; the only fake is none — images are
seeded real bytes.

## Risks & hard problems

- **SurrealDB bucket throughput/size at daily-photo volume** — measure early with a real
  30-day seed; this is the assumption the whole scope stands on.
- **Orphaned uploads** (begun, never committed) — housekeeping sweep with the outbox's
  `next_attempt_ts` pattern.
- **Image decoding is attacker-controlled input** — decode in the variant job (not the
  gateway), with a hardened library and size caps.

## Open questions

- ✅ Chunk size 1 MiB; bearer token auth (no pre-signed URLs — no new auth mechanism).
- ✅ Media sync: node-local v1 (flagged in the sync scope).
- ✅ EXIF: strip by default (the `image` crate's resize already strips metadata; explicit EXIF
  strip is a named follow-up for defense in depth).

## Related

`files-scope.md` · `../document-store/document-store-scope.md` · `../jobs/jobs-scope.md` ·
`../sync/` · `../auth-caps/entity-scoped-grants-scope.md` · the ext-publish body-limit
debug history · first consumer: `cc-app` `docs/scope/care/daily-feed-scope.md`.
