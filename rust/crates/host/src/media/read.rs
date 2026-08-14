//! `media.read` — media BYTES over the MCP bridge, base64, in bounded slices (media scope).
//!
//! ## Why this verb exists
//!
//! Bytes already have a perfectly good path: `GET /media/{id}` over HTTP, with ETag, Range and
//! variants. That route is the right way to reach media and this verb does not replace it.
//!
//! It exists for one caller that route cannot serve: **a module-federated extension UI**.
//!
//! `authenticate()` (`role/gateway/src/session/authenticate.rs`) reads *only*
//! `Authorization: Bearer` — there is no cookie path, and the media route does not accept the
//! `?token=` query param the SSE routes use. So reaching the bytes over HTTP requires a bearer
//! token in the caller's hand. An extension page does not have one and is not supposed to: the
//! host mounts it with a `ctx` carrying `workspace`/`caps`/`route` and a leashed `bridge.call`,
//! deliberately withholding the credential (rubix-ai `ExtHost.tsx`: "it never gets the token"), and
//! page extensions are slated to move behind an iframe sandbox where the ambient session is gone
//! for real rather than by convention.
//!
//! That left extension authors with one working move: reach into the host's `localStorage`, lift
//! the session token, and forge the header themselves. It works today, which is the problem — it
//! silently voids the leash, bypasses the bridge's per-verb scope filter, and breaks the day the
//! sandbox lands. A verb on the bridge is the same bytes through the wall that already exists.
//!
//! ## Shape
//!
//! `{ id, variant?, offset?, limit? }` → `{ id, mime, bytes, offset, len, total, eof, checksum }`
//!
//! **Base64, sliced.** Media runs to 50 MiB (500 MiB for video) and an MCP reply is a JSON payload
//! held in memory on both sides, so returning a whole file would be a denial-of-service with extra
//! steps. `limit` is clamped to [`MAX_READ_BYTES`], and a caller loops on `offset` until `eof` —
//! the same begin/chunk/commit rhythm the upload half already uses, so the idiom is not new.
//!
//! **The gate is `media_serve`'s**, reused rather than restated: the same `store:media/{id}:read`
//! check, the same variant resolution, the same not-ready rule. A second implementation of an
//! authorization check is a second thing to get wrong, and this one guards a customer's bytes.

use lb_auth::Principal;
use lb_store::Store;
use serde_json::{json, Value};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;

use super::error::MediaError;
use super::serve::media_serve;

/// The most bytes one `media.read` call will return, before base64 (1 MiB).
///
/// Matches `CHUNK_SIZE` on the upload side deliberately: a caller that walks a file up in 1 MiB
/// pieces and back down in 1 MiB pieces has one number to reason about. Base64 inflates this ~4/3,
/// so the JSON reply stays comfortably inside a normal payload bound.
pub const MAX_READ_BYTES: usize = 1024 * 1024;

/// Read a slice of media bytes as base64.
///
/// `offset` past the end is **not an error** — it returns an empty slice with `eof: true`, so a
/// caller looping on `eof` terminates cleanly on an empty or exactly-chunk-sized file rather than
/// having to special-case the boundary.
pub async fn media_read(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    variant: Option<&str>,
    offset: u64,
    limit: Option<usize>,
) -> Result<Value, MediaError> {
    // The capability gate, the variant lookup and the not-ready rule all live in `media_serve`.
    // Reusing it is what keeps this verb from becoming a second, subtly different door to the same
    // bytes — the failure mode being a gate that diverges from the HTTP route's under maintenance.
    let served = media_serve(store, principal, ws, id, variant).await?;

    let total = served.bytes.len() as u64;
    let start = offset.min(total) as usize;
    let want = limit.unwrap_or(MAX_READ_BYTES).min(MAX_READ_BYTES);
    let end = start.saturating_add(want).min(served.bytes.len());
    let slice = &served.bytes[start..end];

    Ok(json!({
        "id": id,
        "mime": served.mime,
        "bytes": BASE64.encode(slice),
        "offset": start as u64,
        "len": slice.len() as u64,
        "total": total,
        // The termination signal, computed here rather than left to the caller: `offset + len >=
        // total` is easy to get wrong by one at exactly the boundary that matters, and getting it
        // wrong means either a truncated image or a loop that never ends.
        "eof": (start + slice.len()) as u64 >= total,
        // The ETag the HTTP route would serve, so a caller can cache across mounts and skip the
        // whole read when nothing changed.
        "checksum": served.etag,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The slice arithmetic, isolated from the store. These are the off-by-one cases that produce a
    /// truncated image or a loop that never terminates — both of which look like a broken feature
    /// rather than a bad bound.
    fn slice_of(total: usize, offset: u64, limit: Option<usize>) -> (usize, usize, bool) {
        let start = (offset as usize).min(total);
        let want = limit.unwrap_or(MAX_READ_BYTES).min(MAX_READ_BYTES);
        let end = start.saturating_add(want).min(total);
        let len = end - start;
        (start, len, (start + len) >= total)
    }

    #[test]
    fn a_small_file_comes_back_whole_and_flags_eof() {
        let (start, len, eof) = slice_of(500, 0, None);
        assert_eq!((start, len), (0, 500));
        assert!(eof, "a file inside one slice must terminate the loop immediately");
    }

    #[test]
    fn a_large_file_is_capped_at_the_slice_bound_and_does_not_claim_eof() {
        let (_, len, eof) = slice_of(3 * MAX_READ_BYTES, 0, None);
        assert_eq!(len, MAX_READ_BYTES);
        assert!(!eof);
    }

    /// The exact-multiple boundary — where an off-by-one shows up as either a truncated last chunk
    /// or an infinite loop.
    #[test]
    fn a_file_that_is_exactly_one_slice_terminates() {
        let (_, len, eof) = slice_of(MAX_READ_BYTES, 0, None);
        assert_eq!(len, MAX_READ_BYTES);
        assert!(eof, "exactly one slice is the whole file — eof, not a second empty read");
    }

    #[test]
    fn walking_a_file_reaches_eof_exactly_once() {
        let total = MAX_READ_BYTES * 2 + 17;
        let mut offset = 0u64;
        let mut seen = 0usize;
        let mut rounds = 0;
        loop {
            let (_, len, eof) = slice_of(total, offset, None);
            seen += len;
            offset += len as u64;
            rounds += 1;
            if eof {
                break;
            }
            assert!(rounds < 10, "the walk must terminate");
        }
        assert_eq!(seen, total, "every byte is returned exactly once");
        assert_eq!(rounds, 3);
    }

    /// Past the end is empty + eof, never an error: a caller that loops on `eof` must not have to
    /// special-case the empty file or an over-read.
    #[test]
    fn an_offset_past_the_end_is_empty_and_eof_rather_than_an_error() {
        let (start, len, eof) = slice_of(100, 5_000, None);
        assert_eq!((start, len), (100, 0));
        assert!(eof);
    }

    #[test]
    fn an_empty_file_is_immediately_eof() {
        let (_, len, eof) = slice_of(0, 0, None);
        assert_eq!(len, 0);
        assert!(eof);
    }

    /// A caller asking for more than the bound gets the bound, not the moon.
    #[test]
    fn an_oversized_limit_is_clamped() {
        let (_, len, _) = slice_of(10 * MAX_READ_BYTES, 0, Some(usize::MAX));
        assert_eq!(len, MAX_READ_BYTES);
    }

    #[test]
    fn base64_round_trips_the_exact_bytes() {
        let raw: Vec<u8> = (0u8..=255).collect();
        let encoded = BASE64.encode(&raw);
        assert_eq!(BASE64.decode(encoded).unwrap(), raw);
    }
}
