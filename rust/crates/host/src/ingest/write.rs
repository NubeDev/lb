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

/// The principal-sub prefix an extension acts under. `tool_call::build_call_context` derives every
/// extension call's principal as `ext:{ext_id}`, so a sample written BY an extension is stamped with
/// a producer rooted there. This constant is the grammar, not a name: the host learns "this producer
/// is an extension, and its id is X" without ever knowing which extensions exist.
const EXT_ROOT: &str = "ext:";

/// The authenticated root of a stored producer id — everything before the one separator.
///
/// The inverse of [`root_producer`], and deliberately in the same file: these two are one grammar,
/// and splitting them across modules is how a writer and a reader come to disagree. `root_producer`
/// guarantees at most ONE separator (it collapses declared `/` to `-`), so a plain `split_once` is
/// exact — there is no ambiguity about which slash is the root boundary.
pub fn producer_root(producer: &str) -> &str {
    producer
        .split_once(NS_SEP)
        .map_or(producer, |(root, _)| root)
}

/// The caller-declared leaf beneath the principal root, or `""` when the caller declared nothing.
///
/// This is the producer's OWN id for its stream (modbus writes e.g. `modbus.plant-b@7`), which is
/// what a producer needs handed back to it to answer a question about that specific stream — it
/// never saw the root the host stamped on.
pub fn producer_leaf(producer: &str) -> &str {
    producer.split_once(NS_SEP).map_or("", |(_, leaf)| leaf)
}

/// The extension id a producer is rooted at, or `None` when it is not an extension's stream.
///
/// A producer rooted at `user:test` or an api key has no extension behind it to ask anything of, and
/// that is a first-class answer, not a failure — most series in a workspace are written by humans,
/// flows, or webhooks. Rule 10: the id is READ OUT of the principal grammar, never matched against a
/// list of known extensions.
pub fn producer_ext_id(producer: &str) -> Option<&str> {
    let ext = producer_root(producer).strip_prefix(EXT_ROOT)?;
    (!ext.is_empty()).then_some(ext)
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

    /// The reader is the exact inverse of the writer, for every shape the writer can emit. If these
    /// two ever disagree the producer strip attributes a stream to the wrong extension — or to none.
    #[test]
    fn the_reader_inverts_the_writer() {
        for (sub, declared) in [
            ("ext:modbus", "modbus.sim-net@1784031000"),
            ("ext:modbus", ""),
            ("user:test", "gw-alpha"),
            ("user:test", ""),
            ("apikey:gw-1", "shed-3"),
        ] {
            let stored = root_producer(sub, declared);
            assert_eq!(producer_root(&stored), sub, "root of {stored}");
        }
    }

    #[test]
    fn the_leaf_is_what_the_producer_itself_declared() {
        let stored = root_producer("ext:modbus", "modbus.sim-net@1784031000");
        // What we hand back to the extension: its OWN id for the stream. It never saw the root.
        assert_eq!(producer_leaf(&stored), "modbus.sim-net@1784031000");
        // A caller that declared nothing has no leaf — and "" is not a stream id, it is absence.
        assert_eq!(producer_leaf("ext:modbus"), "");
    }

    /// Rule 10: the id is read OUT of the identity grammar. There is no list of known extensions
    /// anywhere in this function, so a new extension needs no core change to be recognised.
    #[test]
    fn only_an_extension_rooted_producer_yields_an_ext_id() {
        assert_eq!(
            producer_ext_id("ext:modbus/modbus.sim-net@1"),
            Some("modbus")
        );
        assert_eq!(producer_ext_id("ext:modbus"), Some("modbus"));
        assert_eq!(producer_ext_id("ext:weather"), Some("weather"));
        // Humans, agents and api keys write series too — "not an extension" is a first-class answer.
        assert_eq!(producer_ext_id("user:test/gw-alpha"), None);
        assert_eq!(producer_ext_id("agent:reporter"), None);
        assert_eq!(producer_ext_id("apikey:gw-1/shed-3"), None);
        // The prefix ANCHORS — a principal that merely contains "ext:" is not an extension.
        assert_eq!(producer_ext_id("user:ext:sneaky"), None);
        // A bare prefix names no extension.
        assert_eq!(producer_ext_id("ext:"), None);
        assert_eq!(producer_ext_id("ext:/leaf"), None);
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
