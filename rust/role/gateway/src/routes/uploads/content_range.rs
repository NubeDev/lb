//! Parse the `Content-Range: bytes {first}-{last}/{total}` header the resumable `PATCH` frames each
//! chunk with. `{total}` may be `*` (a client streaming an artifact whose size it does not yet know).
//!
//! Its own file because it is the one piece of the upload lane that is pure parsing with real edge
//! cases — and the one a reviewer will want to read without the streaming loop around it.

/// A parsed byte range: `first..=last` of an optional `total`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRange {
    pub first: u64,
    pub last: u64,
    pub total: Option<u64>,
}

impl ByteRange {
    /// How many bytes this range claims to carry. Inclusive bounds, so `0-0` is one byte.
    pub fn len(&self) -> u64 {
        self.last - self.first + 1
    }
}

/// Parse `bytes {first}-{last}/{total}`. Returns `None` for anything malformed — an unsatisfiable
/// range (`last < first`), a non-numeric bound, a missing unit. The caller answers `400`, never a
/// guess: guessing an offset is how a resumable upload silently corrupts an artifact.
pub fn parse(value: &str) -> Option<ByteRange> {
    let rest = value.trim().strip_prefix("bytes")?.trim_start();
    let (range, total) = rest.split_once('/')?;
    let (first, last) = range.trim().split_once('-')?;
    let first: u64 = first.trim().parse().ok()?;
    let last: u64 = last.trim().parse().ok()?;
    if last < first {
        return None;
    }
    let total = match total.trim() {
        "*" => None,
        n => Some(n.parse().ok()?),
    };
    // A range that runs past the declared total is a contradiction in the client's own framing.
    if total.is_some_and(|t| last >= t) {
        return None;
    }
    Some(ByteRange { first, last, total })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_concrete_range() {
        let r = parse("bytes 0-65535/2400000000").expect("valid");
        assert_eq!(r.first, 0);
        assert_eq!(r.last, 65535);
        assert_eq!(r.total, Some(2_400_000_000));
        assert_eq!(r.len(), 65536);
    }

    #[test]
    fn parses_an_unknown_total() {
        assert_eq!(parse("bytes 10-19/*").unwrap().total, None);
    }

    /// Every malformed shape is `None` — the route answers `400` rather than inventing an offset.
    #[test]
    fn refuses_malformed_and_contradictory_ranges() {
        for bad in [
            "",
            "0-10/20",       // no unit
            "bytes 10-5/20", // last < first
            "bytes a-b/20",  // non-numeric
            "bytes 0-10",    // no total
            "bytes 0-20/20", // last >= total
            "items 0-10/20", // wrong unit
        ] {
            assert!(parse(bad).is_none(), "must refuse `{bad}`");
        }
    }
}
