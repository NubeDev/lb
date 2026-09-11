//! The case layer's logical clock — normalize an incoming `ts` to epoch **milliseconds**, and
//! backfill one a door omitted.
//!
//! `lb-cases` is wall-clock-free (testing §3): every `ts` is injected. The host is the single funnel
//! every door reaches, so the normalization lives here — the same guard, and the same three bands,
//! `host/src/insight/raise.rs` applies to `insight.ts`.
//!
//! This is a deliberate small COPY of that guard rather than a re-export: `normalize_ts` there is
//! private and `now_ms` is `pub(super)` to the insight module, and widening another module's
//! visibility to share ten lines would couple two services that only happen to agree today. If the
//! bands ever change they must change in both — which is why both carry the same comment.

/// Epoch-seconds band: any real wall-clock date from ~2001-09 (`1e9`) to ~year 33658 (`1e12`). A
/// `ts` in here was almost certainly stamped in SECONDS (the gateway's `gw.now()` is `as_secs()`).
const TS_SECONDS_MIN: u64 = 1_000_000_000;
/// At or above this a `ts` is already epoch-millis.
const TS_MILLIS_MIN: u64 = 1_000_000_000_000;

/// Normalize a `ts` to epoch milliseconds. `0` ⇒ the host wall-clock. A value in the epoch-seconds
/// band ⇒ ×1000. Everything else (a real ms clock, or a tiny deterministic test/logical clock)
/// passes through unchanged, so tests seeding fixed small clocks stay reproducible.
pub(super) fn normalize_ts(ts: u64) -> u64 {
    if ts == 0 {
        now_ms()
    } else if (TS_SECONDS_MIN..TS_MILLIS_MIN).contains(&ts) {
        ts * 1000
    } else {
        ts
    }
}

/// The host wall-clock as epoch milliseconds — the unit every case timestamp is stored in. Used to
/// backfill a `ts` a browser/CLI door omitted; the crate stays wall-clock-free.
pub(super) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
