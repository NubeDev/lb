//! Construction — `frame(records)` (an array of row maps → a capped Frame) and
//! [`frame_from_grid`], the helper `lb-rules`' `g.frame()` verb calls after the Grid's gated
//! `collect_json()` (the seam already checked workspace + caps; this only shapes + caps rows).

use rhai::{Engine, EvalAltResult};
use serde_json::Value;

use crate::convert::dynamic_to_value;
use crate::json::frame_from_json;
use crate::limits::FrameLimits;
use crate::value::{perr, rerr, Frame};

/// Register the free `frame(records)` constructor.
pub(crate) fn register(engine: &mut Engine, limits: &FrameLimits) {
    let l = *limits;
    engine.register_fn(
        "frame",
        move |records: rhai::Array| -> Result<Frame, Box<EvalAltResult>> {
            let rows: Vec<Value> = records.iter().map(dynamic_to_value).collect();
            if let Some(bad) = rows.iter().find(|r| !r.is_object()) {
                return Err(rerr(format!(
                    "frame(records): every element must be a map of column -> value, got {bad}"
                )));
            }
            let df = frame_from_json(&rows).map_err(perr)?;
            Frame::new(df, l)
        },
    );
}

/// Build a Frame from a collected grid's `columns` + `rows` JSON. Handles BOTH seam wire shapes
/// (like `grid.rs::row_to_map`): platform rows are objects and pass through; federation rows are
/// column-aligned arrays and zip with `columns`. Row cap is enforced BEFORE polars sees a row.
pub fn frame_from_grid(
    columns: &[String],
    rows: &[Value],
    limits: &FrameLimits,
) -> Result<Frame, Box<EvalAltResult>> {
    limits
        .check_frame(rows.len(), columns.len().max(1))
        .map_err(rerr)?;
    let objects: Vec<Value> = rows.iter().map(|r| row_object(r, columns)).collect();
    let df = frame_from_json(&objects).map_err(perr)?;
    Frame::new(df, *limits)
}

/// Normalize one collected row to a JSON object, whatever wire shape produced it.
fn row_object(row: &Value, columns: &[String]) -> Value {
    match row {
        Value::Object(_) => row.clone(),
        Value::Array(cells) => {
            let mut obj = serde_json::Map::with_capacity(cells.len());
            for (i, c) in cells.iter().enumerate() {
                let key = columns
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| format!("col_{i}"));
                obj.insert(key, c.clone());
            }
            Value::Object(obj)
        }
        other => {
            let key = columns.first().cloned().unwrap_or_else(|| "value".into());
            let mut obj = serde_json::Map::with_capacity(1);
            obj.insert(key, other.clone());
            Value::Object(obj)
        }
    }
}
