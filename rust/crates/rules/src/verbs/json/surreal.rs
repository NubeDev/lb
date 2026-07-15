//! SurrealDB-shape normalizers: split a record `Thing` id (`"sensor:abc123"`, including the
//! `⟨angle-bracket⟩`-escaped id form SurrealDB emits for non-plain ids) and `epoch(v)` — the
//! "whatever the source returned" timestamp normalizer (ISO-8601 string | epoch-secs | epoch-ms,
//! number or string → unix SECONDS). The ms/ISO parsing is the chart family's proven normalizer
//! (`verbs/chart.rs::to_epoch_ms`) — one algorithm, one drift surface.

use rhai::{Dynamic, Engine, EvalAltResult};

use crate::grid::rhai_err;

pub(super) fn register(engine: &mut Engine) {
    engine.register_fn("thing_id", |s: &str| split_thing(s).1);
    engine.register_fn("thing_tbl", |s: &str| split_thing(s).0);
    engine.register_fn("epoch", |v: Dynamic| -> Result<i64, Box<EvalAltResult>> {
        epoch_of(&v).ok_or_else(|| {
            rhai_err("epoch: not a timestamp (ISO-8601 string, epoch-secs or epoch-ms)")
        })
    });
}

/// Normalize any source-returned timestamp to epoch seconds (`None` if unparseable). Shared with
/// `rows_epoch` in the rows file.
pub(super) fn epoch_of(d: &Dynamic) -> Option<i64> {
    crate::verbs::chart::to_epoch_ms(d).map(|ms| ms.div_euclid(1000))
}

/// Split `"tbl:id"` at the FIRST colon → `(tbl, id)`, unescaping the id. No colon → the whole
/// string is the id and the table is `""` (a bare id, not a Thing).
fn split_thing(s: &str) -> (String, String) {
    match s.split_once(':') {
        Some((tbl, id)) => (tbl.trim().to_string(), unescape_id(id)),
        None => (String::new(), unescape_id(s)),
    }
}

/// Strip SurrealDB's id escaping: the `⟨…⟩` angle-bracket form (complex ids) and the `` `…` ``
/// backtick form (the alternate escape Surreal accepts).
fn unescape_id(id: &str) -> String {
    let id = id.trim();
    let id = id
        .strip_prefix('⟨')
        .and_then(|x| x.strip_suffix('⟩'))
        .unwrap_or(id);
    let id = id
        .strip_prefix('`')
        .and_then(|x| x.strip_suffix('`'))
        .unwrap_or(id);
    id.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thing_split_plain_and_escaped() {
        // Table: (input, table, id) — incl. the angle-bracket escaped form Surreal emits.
        let cases = [
            ("sensor:abc123", "sensor", "abc123"),
            ("sensor:⟨abc-123⟩", "sensor", "abc-123"),
            ("sensor:⟨01H:X⟩", "sensor", "01H:X"), // id keeps its own colon
            ("sensor:`weird id`", "sensor", "weird id"),
            ("bare", "", "bare"),
        ];
        for (input, tbl, id) in cases {
            assert_eq!(split_thing(input), (tbl.into(), id.into()), "{input}");
        }
    }

    #[test]
    fn epoch_normalizes_the_three_input_shapes() {
        // 2021-01-01 = 18628 days since epoch → 1_609_459_200 secs.
        const T: i64 = 1_609_459_200;
        let cases: [(Dynamic, i64); 5] = [
            (Dynamic::from("2021-01-01T00:00:00Z"), T), // ISO string
            (Dynamic::from_int(T), T),                  // epoch-secs number
            (Dynamic::from_int(T * 1000), T),           // epoch-ms number
            (Dynamic::from(T.to_string()), T),          // epoch-secs as string
            (Dynamic::from((T * 1000).to_string()), T), // epoch-ms as string
        ];
        for (input, want) in cases {
            assert_eq!(epoch_of(&input), Some(want), "input {input:?}");
        }
        assert_eq!(epoch_of(&Dynamic::from("not-a-date")), None);
    }
}
