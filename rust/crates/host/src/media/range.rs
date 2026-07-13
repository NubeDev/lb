//! `media.plan_serve` — resolve the conditional/ranged part of a media GET (media scope): given
//! the served length + ETag and the request's `If-None-Match` / `Range` headers, decide 304 /
//! 200-full / 206-partial / 416. Pure (no store, no HTTP) so the whole matrix is testable
//! without a gateway; the route translates the plan into status + headers.
//!
//! Single-range `bytes=start-end` (plus open-ended `start-` and suffix `-n`) per the scope.
//! Multi-range and malformed specs fall back to a full 200 (RFC 9110 allows ignoring `Range`).
//! `If-None-Match` wins over `Range` (RFC 9110 §13.2.2 evaluation order).

/// How the serve route should respond.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServePlan {
    /// `If-None-Match` matched the ETag → 304, no body.
    NotModified,
    /// No (usable) range → 200 with the full body.
    Full,
    /// A satisfiable single range → 206 with `bytes[start..=end]` + `Content-Range`.
    Partial { start: u64, end: u64 },
    /// A syntactically valid but unsatisfiable range → 416 + `Content-Range: bytes */len`.
    Unsatisfiable,
}

/// Plan the response for a media GET. `len` is the full body length, `etag` the strong ETag
/// (quoted), `if_none_match`/`range` the raw header values if present.
pub fn plan_serve(
    len: u64,
    etag: &str,
    if_none_match: Option<&str>,
    range: Option<&str>,
) -> ServePlan {
    if let Some(inm) = if_none_match {
        if inm.trim() == "*" || inm.split(',').any(|c| c.trim() == etag) {
            return ServePlan::NotModified;
        }
    }
    let Some(spec) = range.and_then(|r| r.strip_prefix("bytes=")) else {
        return ServePlan::Full;
    };
    if spec.contains(',') {
        return ServePlan::Full; // multi-range: serve full (allowed by RFC 9110)
    }
    let Some((a, b)) = spec.trim().split_once('-') else {
        return ServePlan::Full; // malformed: ignore Range
    };
    match (a.is_empty(), b.is_empty()) {
        (true, true) => ServePlan::Full, // "-" alone: malformed
        // Suffix range `-n`: the last n bytes.
        (true, false) => match b.parse::<u64>() {
            Ok(0) => ServePlan::Unsatisfiable,
            Ok(_) if len == 0 => ServePlan::Unsatisfiable,
            Ok(n) => ServePlan::Partial {
                start: len.saturating_sub(n),
                end: len - 1,
            },
            Err(_) => ServePlan::Full,
        },
        // Open-ended `start-`: from start to the end.
        (false, true) => match a.parse::<u64>() {
            Ok(s) if s < len => ServePlan::Partial {
                start: s,
                end: len - 1,
            },
            Ok(_) => ServePlan::Unsatisfiable,
            Err(_) => ServePlan::Full,
        },
        // Bounded `start-end` (inclusive; end clamped to len-1).
        (false, false) => match (a.parse::<u64>(), b.parse::<u64>()) {
            (Ok(s), Ok(e)) if s > e => ServePlan::Full, // malformed: ignore
            (Ok(s), Ok(e)) if s < len => ServePlan::Partial {
                start: s,
                end: e.min(len - 1),
            },
            (Ok(_), Ok(_)) => ServePlan::Unsatisfiable,
            _ => ServePlan::Full,
        },
    }
}
