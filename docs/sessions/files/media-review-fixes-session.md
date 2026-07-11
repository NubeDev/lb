# Session — media peer-review fixes (2026-07-11)

Branch `updates-to-core`. Closed the confirmed peer-review findings against the media scope
(`docs/scope/files/media-scope.md`). Media-scope files only.

## What changed

1. **SECURITY — validated chunk PUT.** New host verb
   `rust/crates/host/src/media/chunk.rs` (`media_chunk_put`): capability
   (`mcp:media.upload:call`) → upload exists → status `Uploading` (kills post-commit
   tampering: a chunk re-PUT after `Ready` changed served bytes under a stale ETag) →
   `n < chunks` → `body ≤ chunk_size` — all **before any byte is written**. The gateway route
   (`rust/role/gateway/src/routes/media.rs`) now calls it and maps errors 403/404/400/413.
   Full write-up: `docs/debugging/files/unchecked-chunk-put-writes.md`.
2. **Corrupt chunk = hard error.** `base64_decode` in `media/model.rs` propagates
   `StoreError::Decode` instead of `unwrap_or_default()` — no more truncated-bytes-with-200.
3. **Range + conditional serve.** New pure planner
   `rust/crates/host/src/media/range.rs` (`plan_serve`): If-None-Match → 304 (wins over
   Range), single-range `bytes=start-end` / `start-` / `-n` → 206 + `Content-Range`,
   unsatisfiable → 416, malformed/multi-range → full 200. Route emits `Accept-Ranges: bytes`.
   Full-buffer read kept for v1; memory caveat recorded in the scope doc.
4. **Multi-chunk resume test** — 3 chunks (2.5 MiB), uploaded out of order, one re-PUT
   (idempotent), commit checksum verifies, serve returns the assembled blob.
5. **Scope honesty** — `media-scope.md` gained a "Deferred — shipped narrower than the
   goals" section: hardcoded limits (no quota/allowlist), inline variant derivation
   (decode-in-gateway risk), no GC/abort + delete never frees chunk rows, no `media.ready`
   event / SDK ticket callbacks, full-buffer serve. Each with the rejected alternative.
6. **Warning cleanup** — unused imports in `media/tool.rs`, `media/variant.rs`, trimmed
   `media/mod.rs` re-exports to what `lb_host` actually exports, dropped dead `media_meta`.

## Tests (all green)

`cargo test -p lb-host --test media_test` → **20 passed** (was 9). New: 6 chunk-put tests
(incl. mandatory cap-deny + ws-isolation on the byte path), corrupt-chunk, multi-chunk
resume, 2 plan_serve unit tests (304 asserted against the real served ETag), range-slice
round-trip. `cargo build -p lb-role-gateway` clean; media module warning-free.
