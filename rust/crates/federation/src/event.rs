//! `emit` — structured, secret-safe events on the federation query path (federation-pool-cache
//! scope). The crate shipped 3,199 lines with ZERO logging, which is why a live incident — one
//! unbounded remote query hung for >2 minutes and wedged the child so hard that *local SQLite*
//! queries also timed out — was invisible from outside until a restart.
//!
//! **The channel is stderr, deliberately.** The supervisor already runs the child with
//! `stderr(Stdio::inherit())` (`crates/supervisor/src/os.rs:37`), so a plain line on stderr reaches
//! the host console with no new plumbing. A `tracing` subscriber in the child would be tidier but
//! buys nothing today: the child is a supervised OS process and cannot reach the host's subscriber
//! or its SurrealDB sink. stdout is NOT available — it carries the `Content-Length`-framed control
//! protocol, and a stray line there would corrupt the frame stream.
//!
//! **What may never appear in an event:** the DSN (or any part of it) and raw SQL. SQL is recorded
//! as `sql_digest` — a SHA-256 prefix plus its length — so two events can be correlated as "the
//! same query" without the text (which routinely carries literals from user data). This mirrors
//! `lb_telemetry::params_digest`'s discipline; see `pool.rs` for why that crate is not linked here.

use sha2::{Digest, Sha256};

/// How a query was resolved against the warm-pool cache.
#[derive(Clone, Copy)]
pub enum Cache {
    Hit,
    Miss,
}

impl Cache {
    fn as_str(self) -> &'static str {
        match self {
            Cache::Hit => "hit",
            Cache::Miss => "miss",
        }
    }
}

/// The outcome of a bounded query.
pub enum Outcome {
    /// Completed; carries the row count.
    Ok(usize),
    /// Exceeded the query bound and was evicted.
    Timeout,
    /// Failed. The message is the source-layer error, which is already DSN-free by contract
    /// (`SourceError`'s doc: the DSN is NEVER included).
    Error(String),
}

/// A digest of `sql` — never the SQL itself. 16 hex chars of SHA-256 plus the length: enough to
/// correlate repeats of one query across events, far too little to reconstruct the text.
pub fn sql_digest(sql: &str) -> String {
    let digest = Sha256::digest(sql.as_bytes());
    let hex: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
    format!("{hex}:{}", sql.len())
}

/// How a query was resolved against the RESULT cache (federation-result-cache scope) — carried
/// alongside the pool `cache` field, which answers a different question (was a *connection* warm).
pub struct ResultCacheEvent {
    pub state: crate::results::ResultCache,
    /// Age of the served entry, on a hit only. This is what lets a UI badge "data as of Xs ago" —
    /// the honest counterweight to the scope's defining risk (stale data that looks live).
    pub age_ms: Option<u128>,
}

/// Emit one query event as a JSON line on stderr.
///
/// `source` is the host-side datasource NAME (an opaque label, not a DSN) — the child receives it
/// for exactly this purpose. Callers that have no name pass `None`.
///
/// `cache` (the pool's state) is `None` — and the field is **OMITTED** — on a result-cache hit: no
/// connect was consulted, so reporting `cache:"hit"` there would imply a pool lookup that never
/// happened. An event must say what the call did, not what is true of the child afterwards.
pub fn query_event(
    source: Option<&str>,
    kind: &str,
    cache: Option<Cache>,
    sql: &str,
    elapsed_ms: u128,
    outcome: &Outcome,
    result_cache: Option<&ResultCacheEvent>,
) {
    let (result, rows, error) = match outcome {
        Outcome::Ok(n) => ("ok", Some(*n), None),
        Outcome::Timeout => ("timeout", None, None),
        Outcome::Error(e) => ("error", None, Some(e.as_str())),
    };
    let mut event = serde_json::json!({
        "evt": "federation.query",
        "kind": kind,
        "sql_digest": sql_digest(sql),
        "elapsed_ms": elapsed_ms as u64,
        "outcome": result,
    });
    if let Some(c) = cache {
        event["cache"] = serde_json::json!(c.as_str());
    }
    if let Some(rc) = result_cache {
        event["result_cache"] = serde_json::json!(rc.state.as_str());
        if let Some(age) = rc.age_ms {
            event["age_ms"] = serde_json::json!(age as u64);
        }
    }
    if let Some(s) = source {
        event["source"] = serde_json::json!(s);
    }
    if let Some(n) = rows {
        event["rows"] = serde_json::json!(n);
    }
    if let Some(e) = error {
        event["error"] = serde_json::json!(e);
    }
    eprintln!("{event}");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The digest identifies a repeat without carrying the text — including any literal inside it.
    #[test]
    fn sql_digest_hides_text_but_correlates() {
        let sql = "SELECT * FROM users WHERE email = 'ada@example.com'";
        let d = sql_digest(sql);
        assert_eq!(d, sql_digest(sql), "same SQL → same digest");
        assert_ne!(d, sql_digest("SELECT 1"), "different SQL → different");
        assert!(!d.contains("ada@example.com"));
        assert!(!d.contains("SELECT"));
        assert!(!d.contains("users"));
    }
}
