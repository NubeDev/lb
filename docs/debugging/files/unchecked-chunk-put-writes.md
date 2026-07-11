# media: chunk PUT was authenticated but uncapped + unvalidated (post-Ready tampering)

**Date:** 2026-07-11 · **Area:** files/media · **Status:** fixed

## Symptom

Peer review of the media scope (branch `updates-to-core`) found that
`PUT /media/{id}/chunk/{n}` (`rust/role/gateway/src/routes/media.rs`) called
`lb_host::chunk_write` directly after `authenticate()` — **no capability check and no
validation against the upload record**. Any authenticated workspace member could:

- write chunks without `mcp:media.upload:call` (the cap was only checked at the
  `upload_begin`/`upload_commit` MCP verbs — the byte path itself was open);
- write chunks to a media id that never began an upload (unbounded junk rows);
- write `n` beyond the declared chunk count, or bodies larger than the chunk size;
- **re-PUT a chunk after commit**: the record stays `Ready`, the served bytes change, and the
  ETag (derived from the commit-time checksum) stays stale — cache-poisoning-grade content
  tampering with a 200 and a "valid" ETag.

A second, related hole: `base64_decode` in `media/model.rs` used `unwrap_or_default()`, so a
corrupt chunk row silently served **truncated bytes with a 200** instead of erroring.

## Root cause

The route treated authentication as authorization and trusted the client to only PUT what
`upload_begin` declared. Validation lived only on the MCP verbs, but the bytes ride HTTP —
the HTTP path needs the same host-mediated gate (rule 5: nothing reachable except through a
capability check).

## Fix

- New host verb `rust/crates/host/src/media/chunk.rs` — `media_chunk_put`: capability
  (`mcp:media.upload:call`) → record exists → status is `Uploading` → `n < chunks` →
  `body ≤ chunk_size`, **all before any byte is written**; the gateway route now calls it and
  maps `MediaError` → 403/404/400/413.
- `base64_decode` now returns `Result` (`StoreError::Decode`); a corrupt chunk row is a 5xx,
  never a truncated 200.

## Regression tests (`rust/crates/host/tests/media_test.rs`)

`chunk_put_denied_without_upload_cap`, `chunk_put_unknown_upload_rejected`,
`chunk_put_after_ready_rejected` (asserts served bytes unchanged),
`chunk_put_out_of_range_n_rejected`, `chunk_put_oversize_body_rejected`,
`chunk_put_isolated_across_workspaces`, `corrupt_chunk_row_is_a_serve_error`.

## Lesson

A raw-bytes HTTP route beside an MCP surface is its own attack surface: the capability +
record-state validation must sit in a host-layer verb the route calls, not on the MCP verbs
around it. "Authenticated" is not "authorized", and post-commit mutability silently breaks
every ETag/caching assumption downstream.
