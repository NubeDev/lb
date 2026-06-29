//! The MCP bridge for the prefs + formatting surface (prefs scope MCP surface). Two tiers:
//!
//!   - **Gated tenant verbs** (`prefs.get/set/resolve/set_default`) — authorized in `verbs.rs`,
//!     reading/writing the caller's (or, for `set_default`, the workspace's) record.
//!   - **Grant-free utility verbs** (`format.datetime/number/quantity`, `convert.unit`) — pure
//!     CLDR/unit math over NO tenant data, so they carry no capability (prefs resolved decision).
//!     The caller passes a fully-resolved `prefs` object (or the individual axes) inline; the host
//!     does not read the store for these, so there is nothing to gate.
//!
//! `call_prefs_tool` dispatches `prefs.*`; `call_format_tool` dispatches `format.*`/`convert.*`. The
//! host `tool_call` bridge routes the prefix here and applies the gate to `prefs.*` only.

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_prefs::{
    convert, format_datetime, format_number, format_quantity, DateStyle, Dimension, NumberFormat,
    NumberOpts, Prefs, ResolvedPrefs, TimeStyle, Unit,
};
use lb_store::Store;
use serde_json::{json, Value};

use super::error::PrefsSvcError;
use super::verbs::{prefs_get, prefs_resolve, prefs_set, prefs_set_default};

/// Dispatch a `prefs.*` verb (gated). The patch/override for `set`/`resolve` is a [`Prefs`] JSON
/// object under `patch`/`override`.
pub async fn call_prefs_tool(
    store: &Store,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "prefs.get" => {
            let prefs = prefs_get(store, principal, ws).await.map_err(svc_err)?;
            Ok(json!({ "prefs": prefs }))
        }
        "prefs.set" => {
            let patch = parse_prefs(input, "patch")?;
            prefs_set(store, principal, ws, &patch)
                .await
                .map_err(svc_err)?;
            Ok(json!({ "ok": true }))
        }
        "prefs.resolve" => {
            let override_ = optional_prefs(input, "override")?;
            let resolved = prefs_resolve(store, principal, ws, override_)
                .await
                .map_err(svc_err)?;
            Ok(json!({ "resolved": resolved }))
        }
        "prefs.set_default" => {
            let patch = parse_prefs(input, "patch")?;
            prefs_set_default(store, principal, ws, &patch)
                .await
                .map_err(svc_err)?;
            Ok(json!({ "ok": true }))
        }
        _ => Err(ToolError::NotFound),
    }
}

/// Dispatch a grant-free `format.*` / `convert.unit` verb. No store, no auth — pure math. The caller
/// supplies the resolved prefs (or axes) inline.
pub fn call_format_tool(qualified_tool: &str, input: &Value) -> Result<Value, ToolError> {
    match qualified_tool {
        "format.datetime" => {
            let instant = i64_arg(input, "instant")?;
            let (tz, date_style, time_style) = datetime_axes(input)?;
            let text = format_datetime(instant, &tz, date_style, time_style)
                .map_err(|e| ToolError::BadInput(e.to_string()))?;
            Ok(json!({ "text": text }))
        }
        "format.number" => {
            let n = f64_arg(input, "value")?;
            let fmt = number_format(input)?;
            let opts = NumberOpts {
                max_frac: input
                    .get("max_frac")
                    .and_then(|v| v.as_u64())
                    .map(|x| x as u8),
            };
            Ok(json!({ "text": format_number(n, fmt, opts) }))
        }
        "format.quantity" => {
            let value = f64_arg(input, "value")?;
            let from = unit_arg(input, "from_unit")?;
            let dimension = dimension_arg(input, "dimension")?;
            let resolved = resolved_arg(input)?;
            let opts = NumberOpts {
                max_frac: input
                    .get("max_frac")
                    .and_then(|v| v.as_u64())
                    .map(|x| x as u8),
            };
            let q = format_quantity(value, from, dimension, &resolved, opts)
                .map_err(|e| ToolError::BadInput(e.to_string()))?;
            Ok(json!({ "text": q.text, "value": q.value, "unit": q.unit }))
        }
        "convert.unit" => {
            let value = f64_arg(input, "value")?;
            let from = unit_arg(input, "from")?;
            let to = unit_arg(input, "to")?;
            let out = convert(value, from, to).map_err(|e| ToolError::BadInput(e.to_string()))?;
            Ok(json!({ "value": out, "unit": to }))
        }
        _ => Err(ToolError::NotFound),
    }
}

// --- argument parsing helpers (one concern: turn JSON into the typed args) ---

fn svc_err(e: PrefsSvcError) -> ToolError {
    match e {
        PrefsSvcError::Denied => ToolError::Denied,
        PrefsSvcError::BadInput(m) => ToolError::BadInput(m),
        PrefsSvcError::Store(_) => ToolError::Denied, // a store failure is opaque at the boundary
    }
}

fn parse_prefs(input: &Value, key: &str) -> Result<Prefs, ToolError> {
    let v = input
        .get(key)
        .ok_or_else(|| ToolError::BadInput(format!("missing arg: {key}")))?;
    serde_json::from_value(v.clone()).map_err(|e| ToolError::BadInput(format!("{key}: {e}")))
}

fn optional_prefs(input: &Value, key: &str) -> Result<Option<Prefs>, ToolError> {
    match input.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => serde_json::from_value(v.clone())
            .map(Some)
            .map_err(|e| ToolError::BadInput(format!("{key}: {e}"))),
    }
}

fn resolved_arg(input: &Value) -> Result<ResolvedPrefs, ToolError> {
    let v = input
        .get("prefs")
        .ok_or_else(|| ToolError::BadInput("missing arg: prefs (resolved)".into()))?;
    serde_json::from_value(v.clone()).map_err(|e| ToolError::BadInput(format!("prefs: {e}")))
}

fn datetime_axes(input: &Value) -> Result<(String, DateStyle, TimeStyle), ToolError> {
    // Either a full resolved `prefs` object, or explicit `timezone`/`date_style`/`time_style`.
    if let Some(p) = input.get("prefs") {
        let r: ResolvedPrefs = serde_json::from_value(p.clone())
            .map_err(|e| ToolError::BadInput(format!("prefs: {e}")))?;
        return Ok((r.timezone, r.date_style, r.time_style));
    }
    let tz = str_arg(input, "timezone")?.to_string();
    let date_style = parse_enum::<DateStyle>(input, "date_style")?;
    let time_style = parse_enum::<TimeStyle>(input, "time_style")?;
    Ok((tz, date_style, time_style))
}

fn number_format(input: &Value) -> Result<NumberFormat, ToolError> {
    if let Some(p) = input.get("prefs") {
        let r: ResolvedPrefs = serde_json::from_value(p.clone())
            .map_err(|e| ToolError::BadInput(format!("prefs: {e}")))?;
        return Ok(r.number_format);
    }
    parse_enum::<NumberFormat>(input, "number_format")
}

fn parse_enum<T: serde::de::DeserializeOwned>(input: &Value, key: &str) -> Result<T, ToolError> {
    let v = input
        .get(key)
        .ok_or_else(|| ToolError::BadInput(format!("missing arg: {key}")))?;
    serde_json::from_value(v.clone()).map_err(|e| ToolError::BadInput(format!("{key}: {e}")))
}

fn unit_arg(input: &Value, key: &str) -> Result<Unit, ToolError> {
    let token = str_arg(input, key)?;
    Unit::parse(token).ok_or_else(|| ToolError::BadInput(format!("unknown unit: {token}")))
}

fn dimension_arg(input: &Value, key: &str) -> Result<Dimension, ToolError> {
    parse_enum::<Dimension>(input, key)
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadInput(format!("missing arg: {key}")))
}

fn f64_arg(input: &Value, key: &str) -> Result<f64, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_f64())
        .ok_or_else(|| ToolError::BadInput(format!("missing numeric arg: {key}")))
}

fn i64_arg(input: &Value, key: &str) -> Result<i64, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_i64())
        .ok_or_else(|| ToolError::BadInput(format!("missing integer arg: {key}")))
}
