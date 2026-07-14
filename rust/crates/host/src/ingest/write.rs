//! `ingest.write` — authorize, stamp the authenticated producer, then durable-append to staging.
//!
//! **The producer is ROOTED at the authenticated calling principal**, and a caller MAY namespace its
//! own streams beneath it: the staged producer is `principal.sub()` when the caller declares nothing,
//! else `{principal.sub()}/{declared}`. The principal prefix is stamped by us and cannot be forged,
//! so the dedup identity `(series, producer, seq)` still cannot be made to collide with or overwrite
//! ANOTHER principal's stream (ingest scope) — a caller can only ever carve up its own namespace.
//!
//! Why the sub-namespace is required, not a nicety: `seq` is monotonic per `(series, producer)` and
//! `series.latest` returns the highest `seq`. Collapsing every stream of one extension onto one flat
//! producer id put them all in ONE seq space — so a producer that restarts (its in-memory `seq`
//! resetting to 0) re-entered the same space below its own high-water mark, and `latest` pinned to
//! the pre-restart sample FOREVER while fresh data landed at lower seqs and never surfaced. The rest
//! of the plane already models multi-producer-per-principal (`commit.rs`: producer-A's seq=5 and
//! producer-B's seq=5 on one series are two rows); only this stamp disagreed.

use lb_auth::Principal;
use lb_ingest::{write as stage_write, Sample};
use lb_store::Store;

use super::authorize::authorize_ingest;
use super::error::IngestError;

/// The default staging bound (max staged rows per workspace) — bounded at the cloud end. A real
/// node folds this into config; the slice fixes a sane default (rate-limiting is out of this slice).
pub const DEFAULT_STAGING_BOUND: usize = 100_000;

/// The separator between the authenticated principal root and a caller-declared sub-namespace.
const NS_SEP: char = '/';

/// Root a caller-declared producer id under the authenticated principal.
///
/// `declared` is UNTRUSTED. The principal root is always stamped by us, so the only thing a caller
/// controls is the leaf beneath its OWN root. We take the declared value verbatim except for the
/// separator itself — a declared `a/b` would otherwise let a caller forge a deeper path or, worse,
/// re-shape its id to mimic another principal's namespace. Separators collapse to `-`.
///
/// An empty/whitespace-only declaration (or one that sanitizes to nothing) means "no sub-namespace":
/// the producer is the bare principal, exactly as before — the back-compatible default.
fn root_producer(principal_sub: &str, declared: &str) -> String {
    let leaf: String = declared
        .trim()
        .chars()
        .map(|c| if c == NS_SEP { '-' } else { c })
        .collect();
    let leaf = leaf.trim().trim_matches('-');
    if leaf.is_empty() {
        principal_sub.to_string()
    } else {
        format!("{principal_sub}{NS_SEP}{leaf}")
    }
}

/// Append `samples` to `ws`'s staging as `principal`. Authorizes `ingest.write` first, then stamps
/// the authenticated producer root onto every sample (preserving any caller-declared sub-namespace
/// beneath it). Returns the count accepted (committed later by the drain worker / `commit_batch`).
pub async fn ingest_write(
    store: &Store,
    principal: &Principal,
    ws: &str,
    samples: Vec<Sample>,
) -> Result<usize, IngestError> {
    authorize_ingest(principal, ws, "ingest.write")?;
    let sub = principal.sub();
    let stamped: Vec<Sample> = samples
        .into_iter()
        .map(|mut s| {
            s.producer = root_producer(sub, &s.producer);
            s
        })
        .collect();
    Ok(stage_write(store, ws, &stamped, DEFAULT_STAGING_BOUND).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_declared_id_is_namespaced_under_the_principal() {
        assert_eq!(
            root_producer("ext:modbus", "modbus.sim-net@1784031000"),
            "ext:modbus/modbus.sim-net@1784031000"
        );
    }

    #[test]
    fn no_declaration_stays_the_bare_principal() {
        // The pre-existing behaviour, preserved: callers that declare nothing are unaffected.
        assert_eq!(root_producer("ext:modbus", ""), "ext:modbus");
        assert_eq!(root_producer("ext:modbus", "   "), "ext:modbus");
    }

    /// The security property the flat stamp existed to guarantee, still held: a caller cannot forge
    /// a producer that escapes its own root and collides with another principal's stream.
    #[test]
    fn a_declared_id_can_never_escape_its_principal_root() {
        for forged in [
            "../ext:other",
            "/ext:other",
            "ext:other/deep",
            "a/b/c",
            "/",
            "///",
        ] {
            let got = root_producer("ext:modbus", forged);
            assert!(
                got.starts_with("ext:modbus"),
                "{forged:?} escaped its root -> {got}"
            );
            assert_eq!(
                got.matches(NS_SEP).count(),
                if got == "ext:modbus" { 0 } else { 1 },
                "{forged:?} forged extra namespace depth -> {got}"
            );
        }
    }

    /// The regression this fixes: two epochs of ONE extension must be DIFFERENT producers, so a
    /// restart's `seq` reset cannot re-enter the pre-restart seq space and pin `series.latest` to a
    /// stale sample forever.
    #[test]
    fn two_epochs_of_one_extension_are_distinct_producers() {
        let before = root_producer("ext:modbus", "modbus.sim-net@1000");
        let after = root_producer("ext:modbus", "modbus.sim-net@2000");
        assert_ne!(before, after);
    }
}
