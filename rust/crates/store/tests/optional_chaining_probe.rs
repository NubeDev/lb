//! `?.` (optional chaining) is gone in SurrealDB 3 — what replaces it?
//!
//! `lb_ingest`'s last-sample lookup reads
//! `(SELECT ts, seq FROM ONLY latest:$s)?.{ ts: ts, seq: seq } ?? { ts: -1, seq: -1 }`.
//! SurrealDB 3's lexer has no `?.` token (`syn/lexer/byte.rs` produces a bare `?` before a `.`), so
//! the parser reports "Unexpected token `?`, expected `??`" and every commit fails.
//!
//! The `??` fallback still exists. The open question is whether a plain `.` projection is
//! NONE-safe on its own — if it is, the `?` simply goes away. Probed, not assumed, and kept as a
//! regression test so a future engine change is caught here rather than in the ingest path.

use lb_store::Store;
use serde_json::Value;

fn parses(sql: &str) -> Result<(), String> {
    surrealdb_core::syn::parse(sql)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

const OLD: &str =
    "LET $c = (SELECT ts, seq FROM ONLY l:x)?.{ ts: ts, seq: seq } ?? { ts: -1, seq: -1 };";
const PLAIN_DOT: &str =
    "LET $c = (SELECT ts, seq FROM ONLY l:x).{ ts: ts, seq: seq } ?? { ts: -1, seq: -1 };";

#[test]
fn the_old_optional_chaining_form_no_longer_parses() {
    let e = parses(OLD).expect_err("`?.` must be gone");
    assert!(e.contains("`??`"), "unexpected error: {e}");
}

#[test]
fn a_plain_dot_projection_parses() {
    parses(PLAIN_DOT).expect("`.{ … }` must still parse");
}

/// Parsing is not the point — the fallback has to fire when the row is absent and the real values
/// must come through when it is present. Both halves, on the real engine, for each candidate.
#[tokio::test]
async fn which_replacement_actually_behaves_like_the_old_form() {
    let store = Store::memory().await.expect("open");
    store
        .query_ws("ws-a", "CREATE l:here SET ts = 7, seq = 3", vec![])
        .await
        .expect("seed");

    // A plain `.` is NOT a drop-in: on a missing row it yields NULL, and `??` only substitutes for
    // NONE, so the default never fires. Field-wise `??` keeps NONE semantics all the way down.
    let candidates = [
        // NB: these are plain `&str` templates used with `replace`, not `format!` — single braces.
        ("plain-dot", "(SELECT ts, seq FROM ONLY l:ID).{ ts: ts, seq: seq } ?? { ts: -1, seq: -1 }"),
        ("field-wise", "{ ts: (SELECT ts, seq FROM ONLY l:ID).ts ?? -1, seq: (SELECT ts, seq FROM ONLY l:ID).seq ?? -1 }"),
    ];

    for (name, tmpl) in candidates {
        for (id, want_ts) in [("missing", -1i64), ("here", 7i64)] {
            let expr = tmpl.replace("ID", id);
            let sql = format!("LET $c = {expr}; RETURN $c;");
            let mut resp = store.query_ws("ws-a", &sql, vec![]).await.expect("run");
            let got: Vec<Value> = resp.take(1).expect("take RETURN");
            let ts = got
                .first()
                .and_then(|v| v.get("ts"))
                .and_then(|v| v.as_i64());
            println!("{name:12} {id:8} -> ts={ts:?} (want {want_ts})  raw={got:?}");
        }
    }

    // The form lb will adopt must satisfy BOTH cases.
    for (id, want_ts, want_seq) in [("missing", -1i64, -1i64), ("here", 7, 3)] {
        let sql = format!(
            "LET $c = {{ ts: (SELECT ts, seq FROM ONLY l:{id}).ts ?? -1, \
                          seq: (SELECT ts, seq FROM ONLY l:{id}).seq ?? -1 }}; RETURN $c;"
        );
        let mut resp = store.query_ws("ws-a", &sql, vec![]).await.expect("run");
        let got: Vec<Value> = resp.take(1).expect("take");
        let row = got.first().expect("a row");
        assert_eq!(
            row.get("ts").and_then(|v| v.as_i64()),
            Some(want_ts),
            "ts for {id}"
        );
        assert_eq!(
            row.get("seq").and_then(|v| v.as_i64()),
            Some(want_seq),
            "seq for {id}"
        );
    }
}
