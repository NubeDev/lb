//! `parse_json` / `to_json` / `to_json_pretty` — the string ↔ value codec, bridged through the
//! crate's one serde_json ↔ `Dynamic` seam (`crate::grid`). JSON `null` ↔ rhai `()` both ways.

use rhai::{Dynamic, Engine, EvalAltResult};

use crate::grid::{dynamic_to_json, json_to_dynamic, rhai_err};

pub(super) fn register(engine: &mut Engine) {
    engine.register_fn(
        "parse_json",
        |s: &str| -> Result<Dynamic, Box<EvalAltResult>> {
            let v: serde_json::Value =
                serde_json::from_str(s).map_err(|e| rhai_err(format!("parse_json: {e}")))?;
            Ok(json_to_dynamic(&v))
        },
    );
    engine.register_fn(
        "to_json",
        |v: Dynamic| -> Result<String, Box<EvalAltResult>> {
            serde_json::to_string(&dynamic_to_json(&v))
                .map_err(|e| rhai_err(format!("to_json: {e}")))
        },
    );
    engine.register_fn(
        "to_json_pretty",
        |v: Dynamic| -> Result<String, Box<EvalAltResult>> {
            serde_json::to_string_pretty(&dynamic_to_json(&v))
                .map_err(|e| rhai_err(format!("to_json_pretty: {e}")))
        },
    );
}

#[cfg(test)]
mod tests {
    use crate::grid::{dynamic_to_json, json_to_dynamic};

    #[test]
    fn null_round_trips_as_unit() {
        let v: serde_json::Value = serde_json::json!({ "a": null, "b": [1, null] });
        let d = json_to_dynamic(&v);
        assert_eq!(dynamic_to_json(&d), v);
    }
}
