//! The slice-D **classification table** (agent-loop-hardening scope): status × headers × overflow
//! discriminant → lane, pinned row by row so a lane change is a deliberate diff, never drift.
//! Pure — the fault type classifies on structured evidence only; the wire-level construction
//! (header parsing, overflow body detection) is covered by `openai_compat_test.rs` over real HTTP.

use lb_role_ai_gateway::{FaultLane, ProviderFault};

#[test]
fn the_classification_table_holds() {
    use FaultLane::*;
    let table: Vec<(ProviderFault, FaultLane, &str)> = vec![
        // Connection-level faults: always transient (retry may reach a healed network).
        (ProviderFault::network("conn refused"), Transient, "network"),
        (ProviderFault::timeout("timed out"), Transient, "timeout"),
        // A 2xx with an unreadable body — bounded retry (a proxy hiccup), fatal after the ceiling.
        (ProviderFault::malformed("bad json"), Transient, "malformed"),
        // Rate limiting / server-side: transient, with or without Retry-After.
        (ProviderFault::http(429, None, "x"), Transient, "429"),
        (
            ProviderFault::http(429, Some(2), "x"),
            Transient,
            "429 + retry-after",
        ),
        (ProviderFault::http(408, None, "x"), Transient, "408"),
        (ProviderFault::http(500, None, "x"), Transient, "500"),
        (ProviderFault::http(502, None, "x"), Transient, "502"),
        (ProviderFault::http(503, None, "x"), Transient, "503"),
        (
            ProviderFault::http(503, Some(30), "x"),
            Transient,
            "503 + retry-after",
        ),
        (ProviderFault::http(504, None, "x"), Transient, "504"),
        (ProviderFault::http(599, None, "x"), Transient, "599"),
        // Auth / malformed request / wrong endpoint: a verbatim retry cannot succeed — fatal.
        (ProviderFault::http(400, None, "x"), Fatal, "plain 400"),
        (ProviderFault::http(401, None, "x"), Fatal, "401 auth"),
        (ProviderFault::http(403, None, "x"), Fatal, "403 forbidden"),
        (ProviderFault::http(404, None, "x"), Fatal, "404 endpoint"),
        (ProviderFault::http(422, None, "x"), Fatal, "422"),
        // Even a Retry-After on a fatal status does not make it retryable.
        (
            ProviderFault::http(403, Some(5), "x"),
            Fatal,
            "403 + retry-after stays fatal",
        ),
        // The overflow lane: the structured body code (any status) or a 413.
        (
            ProviderFault::overflow(400, "ctx"),
            Overflow,
            "400 + context_length_exceeded",
        ),
        (ProviderFault::http(413, None, "x"), Overflow, "413"),
        (
            ProviderFault::overflow(429, "ctx"),
            Overflow,
            "overflow discriminant beats the 429 status",
        ),
    ];

    for (fault, want, row) in table {
        assert_eq!(fault.lane(), want, "row: {row} ({fault:?})");
    }
}
