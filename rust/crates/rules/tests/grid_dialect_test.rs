//! Grid helpers must work on BOTH seam kinds — the coverage gap that hid three bugs at once
//! (`docs/debugging/rules/grid-size-surrealql-on-federation.md`).
//!
//! The two seams differ in two independent ways, and `grid.rs` handled each in exactly one place:
//!
//! | | Platform (SurrealDB) | Federation (DataFusion → sqlite/postgres) |
//! |---|---|---|
//! | count dialect | `count()` + `GROUP ALL` | `COUNT(*)`, and `GROUP ALL` is a parse error |
//! | row shape | JSON objects `{"v":3}` | column-aligned arrays `[3]` |
//!
//! `size()` shipped hard-coding the platform dialect AND the platform row shape, so it was a parse
//! error on federation and — once the SQL was fixed — silently returned **0**. `ai.classify` then
//! dropped its labels for the same row-shape reason. All three are one root: the federation shape was
//! handled in `records()` and nowhere else.
//!
//! **Why the existing suite could not catch this:** `tests/support/mod.rs`'s seam keys its count
//! branch on `query.contains("count()")` — the SurrealQL spelling — and returns object rows for both
//! kinds. It bakes in the platform assumption it is meant to be testing. This file uses a seam that
//! models each backend faithfully instead.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use lb_rules::grid::{Grid, GridCtx};
use lb_rules::seam::{DataSeam, SchemaColumn, SourceKind};
use lb_rules::GridJson;

/// A seam that answers the way each backend REALLY does — the right dialect, the right row shape.
struct TwoBackends {
    seen: Mutex<Vec<String>>,
    n: i64,
}

impl DataSeam for TwoBackends {
    fn resolve(&self, source: &str) -> Result<(SourceKind, String), String> {
        match source {
            "store" => Ok((SourceKind::Platform, source.into())),
            "readings" => Ok((SourceKind::Federation, source.into())),
            other => Err(format!("source not allowed: {other}")),
        }
    }

    fn collect(&self, kind: SourceKind, _source: &str, query: &str) -> Result<GridJson, String> {
        self.seen.lock().unwrap().push(query.to_string());
        match kind {
            // SurrealDB: `COUNT(*)` is a parse error, `count()`/`GROUP ALL` is the spelling; rows are
            // OBJECTS.
            SourceKind::Platform => {
                if query.contains("COUNT(*)") {
                    return Err("Unexpected token `*`, expected an expression".into());
                }
                Ok(GridJson {
                    columns: vec!["v".into()],
                    rows: vec![serde_json::json!({ "v": self.n })],
                })
            }
            // DataFusion: `GROUP ALL` is a parse error; rows are column-aligned ARRAYS.
            SourceKind::Federation => {
                if query.contains("GROUP ALL") || query.contains("count()") {
                    return Err("Expected: end of statement, found: GROUP".into());
                }
                Ok(GridJson {
                    columns: vec!["v".into()],
                    rows: vec![serde_json::json!([self.n])],
                })
            }
        }
    }

    fn schemas(&self) -> Result<BTreeMap<String, Vec<SchemaColumn>>, String> {
        Ok(BTreeMap::new())
    }
}

fn grid(kind: SourceKind, source: &str, n: i64) -> (Grid, Arc<TwoBackends>) {
    let seam = Arc::new(TwoBackends {
        seen: Mutex::new(Vec::new()),
        n,
    });
    let ctx = Arc::new(GridCtx {
        data: seam.clone() as Arc<dyn DataSeam>,
    });
    (
        Grid::new(kind, source.into(), "SELECT a FROM t".into(), ctx),
        seam,
    )
}

/// **The regression.** `size()` on a federation source was a hard parse error — the SurrealQL
/// `count() … GROUP ALL` reaching DataFusion — which also made `ai.classify` unusable on every
/// time-series source, since its over-large-grid guard calls `size()` first.
#[test]
fn size_speaks_sql_on_a_federation_source() {
    let (g, seam) = grid(SourceKind::Federation, "readings", 1152);
    let n = g
        .size()
        .expect("federation size() must not be a parse error");
    assert_eq!(n, 1152, "and it must read the count out of an ARRAY row");
    let sql = seam.seen.lock().unwrap().join(" | ");
    assert!(
        !sql.contains("GROUP ALL") && !sql.contains("count()"),
        "no SurrealQL may reach a federation source: {sql}"
    );
}

/// The other half of the dispatch: the platform path must keep its SurrealQL. A "portable" one-liner
/// (`COUNT(*)`, no `GROUP ALL`) fixes federation and BREAKS this — which is why the fix dispatches
/// rather than picking a spelling.
#[test]
fn size_keeps_surrealql_on_the_platform_source() {
    let (g, seam) = grid(SourceKind::Platform, "store", 5);
    assert_eq!(g.size().expect("platform size() still works"), 5);
    let sql = seam.seen.lock().unwrap().join(" | ");
    assert!(
        sql.contains("count()") && sql.contains("GROUP ALL"),
        "the platform path needs the SurrealQL spelling: {sql}"
    );
}

/// The quieter half of the bug: with the SQL fixed, an object-only read (`r.get("v")`) returned 0 on
/// federation — a WRONG ANSWER, not an error. Zero rows reads as "always under the cap", which
/// defeats `ai.classify`'s over-large-grid guard entirely.
#[test]
fn a_federation_count_is_never_silently_zero() {
    let (g, _) = grid(SourceKind::Federation, "readings", 42);
    assert_eq!(
        g.size().unwrap(),
        42,
        "an array row must not read as 0 — a wrong count silently defeats the classify cap"
    );
}
