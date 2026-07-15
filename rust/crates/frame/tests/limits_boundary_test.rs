//! The governors + the NaN/null boundary (tests mirror `limits.rs` + `value.rs` + the export
//! paths): over-cap construction fails, an exploding join fails on the OUTPUT cap, the string
//! exports respect `max_string_bytes`, and NaN/Inf normalize to null at every export boundary.

mod support;

use lb_frame::{frame_from_grid, FrameLimits};
use serde_json::json;
use support::{engine, engine_with, eval_err, eval_json};

fn tight(rows: usize, cells: usize, bytes: usize) -> FrameLimits {
    FrameLimits {
        max_frame_rows: rows,
        max_frame_cells: cells,
        max_string_bytes: bytes,
    }
}

#[test]
fn over_cap_construct_fails_with_a_clear_error() {
    let e = engine_with(tight(2, 100, 4096));
    let err = eval_err(&e, "frame([#{ x: 1 }, #{ x: 2 }, #{ x: 3 }])");
    assert!(err.contains("max_frame_rows (2)"), "got: {err}");
    assert!(err.contains("3 rows"), "got: {err}");
}

#[test]
fn cell_cap_bounds_wide_frames() {
    let e = engine_with(tight(100, 4, 4096));
    // 2 rows x 3 cols = 6 cells > 4.
    let err = eval_err(&e, "frame([#{ a: 1, b: 2, c: 3 }, #{ a: 4, b: 5, c: 6 }])");
    assert!(err.contains("max_frame_cells (4)"), "got: {err}");
}

#[test]
fn frame_from_grid_checks_rows_before_polars() {
    let limits = tight(1, 100, 4096);
    let cols = vec!["x".to_string()];
    let rows = vec![json!({ "x": 1 }), json!({ "x": 2 })];
    let err = frame_from_grid(&cols, &rows, &limits)
        .err()
        .expect("must fail");
    assert!(err.to_string().contains("max_frame_rows (1)"), "got: {err}");
}

#[test]
fn frame_from_grid_zips_federation_positional_rows() {
    let limits = FrameLimits::default();
    let cols = vec!["building".to_string(), "kwh".to_string()];
    let rows = vec![json!(["Riverside", 4.68]), json!(["Westend", 0.79])];
    let f = frame_from_grid(&cols, &rows, &limits).expect("build");
    assert_eq!(f.df().shape(), (2, 2));
    let out = lb_frame::frame_to_json(f.df()).unwrap();
    assert_eq!(out[0]["building"], json!("Riverside"));
    assert_eq!(out[1]["kwh"], json!(0.79));
}

#[test]
fn exploding_join_fails_on_the_output_cap() {
    // 20x20 many-to-many join -> 400 rows > 50: the OUTPUT check fires (the honest bound; the
    // inputs alone look innocent).
    let e = engine_with(tight(50, 10_000, 4096));
    let script = r#"
        let rows = [];
        for i in 0..20 { rows.push(#{ k: "same", i: i }); }
        let f = frame(rows);
        f.join(f, "k", "inner")
    "#;
    let err = eval_err(&e, script);
    assert!(err.contains("max_frame_rows (50)"), "got: {err}");
    assert!(err.contains("400 rows"), "got: {err}");
}

#[test]
fn vstack_and_sql_outputs_are_capped_too() {
    let e = engine_with(tight(3, 10_000, 4096));
    let base = "let f = frame([#{ x: 1 }, #{ x: 2 }]);";
    let err = eval_err(&e, &format!("{base} f.vstack(f)"));
    assert!(err.contains("max_frame_rows (3)"), "got: {err}");
    let err = eval_err(
        &e,
        &format!("{base} f.sql(\"SELECT a.x FROM self a CROSS JOIN self b\")"),
    );
    assert!(err.contains("max_frame_rows (3)"), "got: {err}");
}

#[test]
fn string_exports_respect_max_string_bytes() {
    let e = engine_with(tight(1000, 10_000, 64));
    let script = r#"
        let rows = [];
        for i in 0..50 { rows.push(#{ name: "row-number-" + i }); }
        frame(rows)
    "#;
    let err = eval_err(&e, &format!("{script}.to_csv_string()"));
    assert!(err.contains("max_string_bytes (64)"), "got: {err}");
    let err = eval_err(&e, &format!("{script}.to_json_string()"));
    assert!(err.contains("max_string_bytes (64)"), "got: {err}");
    // Under the cap both succeed.
    let ok = eval_json(&e, r#"frame([#{ x: 1 }]).to_csv_string()"#).unwrap();
    assert_eq!(ok, json!("x\n1\n"));
}

#[test]
fn nan_and_inf_normalize_to_null_at_every_export() {
    let e = engine();
    // 0.0/0.0 is NaN, 1.0/0.0 is +Inf — both must come back as () / null.
    let script = r#"
        let f = frame([#{ x: 0.0/0.0 }, #{ x: 1.0/0.0 }, #{ x: 2.5 }]);
    "#;
    assert_eq!(
        eval_json(&e, &format!("{script} f.col(\"x\")")).unwrap(),
        json!([null, null, 2.5])
    );
    assert_eq!(
        eval_json(&e, &format!("{script} f.records()")).unwrap(),
        json!([{ "x": null }, { "x": null }, { "x": 2.5 }])
    );
    assert_eq!(
        eval_json(&e, &format!("{script} f.to_json_string()")).unwrap(),
        json!("[{\"x\":null},{\"x\":null},{\"x\":2.5}]")
    );
    assert_eq!(
        eval_json(&e, &format!("{script} f.to_csv_string()")).unwrap(),
        json!("x\n\n\n2.5\n")
    );
    // A NaN produced INSIDE polars (0/0 division) also exports as null.
    let inner = r#"
        frame([#{ a: 0.0, b: 0.0 }])
            .sql("SELECT a / b AS r FROM self")
            .col("r")
    "#;
    assert_eq!(eval_json(&e, inner).unwrap(), json!([null]));
}

#[test]
fn to_grid_json_exports_the_chart_shape() {
    let e = engine();
    let out = eval_json(&e, r#"frame([#{ ts: 1, v: 2.0 }]).to_grid_json()"#).unwrap();
    assert_eq!(out["columns"], json!(["ts", "v"]));
    assert_eq!(out["rows"], json!([{ "ts": 1, "v": 2.0 }]));
}

#[test]
fn csv_quotes_rfc4180() {
    let e = engine();
    let out = eval_json(
        &e,
        r#"frame([#{ s: "a,b", t: "say \"hi\"" }]).to_csv_string()"#,
    )
    .unwrap();
    assert_eq!(out, json!("s,t\n\"a,b\",\"say \"\"hi\"\"\"\n"));
}
