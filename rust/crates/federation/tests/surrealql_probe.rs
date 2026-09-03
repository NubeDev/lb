//! Can DataFusion's sqlparser classify SurrealQL well enough to be a read-only gate?
//! False REJECT is annoying. False ACCEPT — a mutation parsed as a read — is a security hole.
use datafusion::sql::sqlparser::dialect::{GenericDialect, PostgreSqlDialect};
use datafusion::sql::sqlparser::parser::Parser;

fn kind(sql: &str) -> String {
    for (name, r) in [
        ("pg", Parser::parse_sql(&PostgreSqlDialect {}, sql)),
        ("generic", Parser::parse_sql(&GenericDialect {}, sql)),
    ] {
        match r {
            Ok(st) if st.len() == 1 => {
                let d = format!("{:?}", st[0]);
                return format!(
                    "{name}: {}",
                    d.split(['{', '(']).next().unwrap_or("?").trim()
                );
            }
            Ok(st) => return format!("{name}: {} statements", st.len()),
            Err(_) => continue,
        }
    }
    "PARSE FAILED in both dialects".into()
}

#[test]
fn what_does_sqlparser_make_of_real_surrealql() {
    let cases = [
        ("plain read",        "SELECT * FROM t"),
        ("lb's real read",    "SELECT series, producer, seq, time::millis(ts) AS ts, payload FROM series WHERE series = $series ORDER BY seq ASC"),
        ("lb's real write",   "UPSERT type::record('series', [$se, $pr, $sq]) CONTENT { series: $se, ts: $ts }"),
        ("delete",            "DELETE type::record('ingest_staging', [$se, $pr, $sq])"),
        ("namespace escape",  "USE NS other DB other; SELECT * FROM t"),
        ("schema change",     "DEFINE INDEX series_ts_idx ON series FIELDS series, ts"),
        ("mutation in a read","SELECT * FROM (UPDATE person SET x = 1)"),
        ("record id literal", "SELECT * FROM series:['modbus.a', 'p', 1]"),
        ("FROM ONLY",         "SELECT ts FROM ONLY series_latest:['x']"),
    ];
    for (label, sql) in cases {
        println!("{label:20} -> {}", kind(sql));
    }
}
