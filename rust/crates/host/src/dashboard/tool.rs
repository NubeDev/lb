//! The MCP bridge for dashboard verbs — host-native tools under the one MCP contract (README §6.5).
//! UI, agents, and extensions reach `dashboard.*` the SAME way they reach any wasm tool: a qualified
//! call with JSON in/out. The MCP gate runs inside each verb FIRST (workspace-first, then
//! `mcp:dashboard.<verb>:call`), so a ws-B caller or one without the grant is refused before the verb
//! runs (the mandatory deny + isolation tests are real here). Host-native — not in the runtime
//! `Registry`; the gateway routes `dashboard.*` here for the routed/agent path.
//!
//! `save`/`delete`/`share` take their logical `now` from the args (the caller's clock — determinism
//! §3, never wall-clock in the verb), exactly as `assets.put_doc` takes `ts`.

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::{json, Value};

use lb_authz::Subject;

use super::model::{Cell, Toolbar, Visibility};
use super::{
    dashboard_access_check, dashboard_delete, dashboard_get, dashboard_list, dashboard_pin,
    dashboard_save_meta, dashboard_share, DashboardError,
};

/// Dispatch a `dashboard.<verb>` MCP call. `input` is the verb's JSON arguments; the return is the
/// verb's JSON result. Each verb authorizes first; denials are opaque (`ToolError::Denied`).
pub async fn call_dashboard_tool(
    store: &Store,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "dashboard.get" => {
            let d = dashboard_get(store, principal, ws, str_arg(input, "id")?)
                .await
                .map_err(to_tool)?;
            Ok(serde_json::to_value(d).unwrap_or(Value::Null))
        }
        "dashboard.list" => {
            let rows = dashboard_list(store, principal, ws)
                .await
                .map_err(to_tool)?;
            Ok(json!({ "dashboards": rows }))
        }
        "dashboard.save" => {
            let cells: Vec<Cell> = typed_arg(arg(input, "cells")?, "cells")?;
            // `variables` is additive — a pre-variables caller omits it (defaults to empty).
            let variables = match input.get("variables") {
                Some(v) if !v.is_null() => typed_arg(v, "variables")?,
                _ => Vec::new(),
            };
            // Page presentation (dashboard page-settings) — additive & optional. An ABSENT key
            // preserves the stored value (the settings dialog is the only writer; a layout/variable
            // save omits them). `opt_str_arg` maps a present-and-string arg to `Some(_)`.
            let d = dashboard_save_meta(
                store,
                principal,
                ws,
                str_arg(input, "id")?,
                str_arg(input, "title")?,
                opt_str_arg(input, "description"),
                opt_str_arg(input, "icon"),
                opt_str_arg(input, "color"),
                opt_toolbar_arg(input),
                cells,
                variables,
                u64_arg(input, "now")?,
            )
            .await
            .map_err(to_tool)?;
            Ok(serde_json::to_value(d).unwrap_or(Value::Null))
        }
        "dashboard.pin" => {
            // widget-platform scope, Slice B — mint a cell from an `x-lb-render` envelope and upsert it
            // into a dashboard. `envelope` is the opaque render envelope (a descriptor.result or a channel
            // rich_result body minus kind/v); `dashboard` is the target id (idempotent UPSERT,
            // owner-only update). Gated by `mcp:dashboard.pin:call` (its own cap, distinct from .save).
            let envelope = arg(input, "envelope")?.clone();
            let d = dashboard_pin(
                store,
                principal,
                ws,
                str_arg(input, "dashboard")?,
                input.get("title").and_then(Value::as_str).unwrap_or(""),
                &envelope,
                u64_arg(input, "now")?,
            )
            .await
            .map_err(to_tool)?;
            Ok(serde_json::to_value(d).unwrap_or(Value::Null))
        }
        "dashboard.access_check" => {
            // access-model scope: the read-only dependency-closure preflight. `dashboard` is the id;
            // `subject`/`team` names WHOSE reach to check (defaults to the caller for a self-preflight).
            // The subject string is a `user:`/`team:` handle parsed into a `Subject`.
            let dashboard_id = str_arg(input, "dashboard").or_else(|_| str_arg(input, "id"))?;
            let subject_str = input
                .get("subject")
                .or_else(|| input.get("team"))
                .and_then(Value::as_str)
                .unwrap_or(principal.sub());
            let subject = Subject::parse(subject_str).ok_or_else(|| {
                ToolError::BadInput(format!(
                    "bad subject `{subject_str}` — expected user:<name> or team:<name>"
                ))
            })?;
            let report = dashboard_access_check(store, principal, ws, dashboard_id, &subject)
                .await
                .map_err(to_tool)?;
            Ok(serde_json::to_value(report).unwrap_or(Value::Null))
        }
        "dashboard.delete" => {
            dashboard_delete(
                store,
                principal,
                ws,
                str_arg(input, "id")?,
                u64_arg(input, "now")?,
            )
            .await
            .map_err(to_tool)?;
            Ok(json!({ "ok": true }))
        }
        "dashboard.share" => {
            let visibility = visibility_arg(input)?;
            let team = input.get("team").and_then(|v| v.as_str());
            let d = dashboard_share(
                store,
                principal,
                ws,
                str_arg(input, "id")?,
                visibility,
                team,
                u64_arg(input, "now")?,
            )
            .await
            .map_err(to_tool)?;
            Ok(serde_json::to_value(d).unwrap_or(Value::Null))
        }
        _ => Err(ToolError::NotFound),
    }
}

/// Map the dashboard gate's outcome onto the MCP tool error (denials opaque).
fn to_tool(e: DashboardError) -> ToolError {
    match e {
        DashboardError::Denied => ToolError::Denied,
        DashboardError::NotFound => ToolError::NotFound,
        DashboardError::BadInput(m) => ToolError::BadInput(m),
        DashboardError::Store(s) => ToolError::Extension(s.to_string()),
    }
}

fn arg<'a>(input: &'a Value, key: &str) -> Result<&'a Value, ToolError> {
    input
        .get(key)
        .ok_or_else(|| ToolError::BadInput(format!("missing arg: {key}")))
}

/// Decode a structured arg, tolerating the JSON-encoded-STRING form (`"[{…}]"` instead of `[{…}]`)
/// AI callers routinely emit — the live agent sent stringified `cells` five turns in a row and the
/// plain type error never steered it. Decoding the string costs nothing in authority: the verb's
/// own validators (bounds/views/genui/refs) still run on the decoded value. A string that is not
/// valid JSON of the target type errors with a message that names the right encoding.
fn typed_arg<T: serde::de::DeserializeOwned>(v: &Value, key: &str) -> Result<T, ToolError> {
    let v = match v {
        Value::String(s) => serde_json::from_str::<Value>(s).map_err(|_| {
            ToolError::BadInput(format!(
                "{key}: arrived as a string that is not valid JSON — pass a JSON array, not a JSON-encoded string"
            ))
        })?,
        other => other.clone(),
    };
    serde_json::from_value(v).map_err(|e| ToolError::BadInput(format!("{key}: {e}")))
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    arg(input, key)?
        .as_str()
        .ok_or_else(|| ToolError::BadInput(format!("arg not a string: {key}")))
}

/// An OPTIONAL string arg: `Some` when the key is present and a string, `None` when absent or
/// explicit `null` (the "preserve the stored value" signal — page-settings fields). A present
/// non-string is coerced to `None` too (lenient — no reason to fail a whole save over it).
fn opt_str_arg(input: &Value, key: &str) -> Option<String> {
    input.get(key).and_then(Value::as_str).map(str::to_string)
}

/// The OPTIONAL `toolbar` arg (dashboard toolbar-settings): the header-chrome visibility flags.
/// `Some` when present as an object (the settings dialog is the only writer), `None` when absent or
/// null — the "preserve the stored flags" signal, exactly like the page-settings string fields. A
/// present-but-malformed value is coerced to `None` (lenient — never fail a whole save over chrome).
fn opt_toolbar_arg(input: &Value) -> Option<Toolbar> {
    match input.get("toolbar") {
        Some(v) if !v.is_null() => serde_json::from_value(v.clone()).ok(),
        _ => None,
    }
}

/// A u64 arg, tolerating the numeric-STRING form (`"1783235133"`) AI callers routinely emit —
/// live, `dashboard.share` failed its whole run on `now` arriving as a string. The steering
/// message names the expected encoding.
fn u64_arg(input: &Value, key: &str) -> Result<u64, ToolError> {
    let v = arg(input, key)?;
    v.as_u64()
        .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
        .ok_or_else(|| {
            ToolError::BadInput(format!(
                "arg not a u64: {key} — pass unix epoch seconds as a JSON number"
            ))
        })
}

/// Parse the `visibility` arg (`"private" | "team" | "workspace"`).
fn visibility_arg(input: &Value) -> Result<Visibility, ToolError> {
    match str_arg(input, "visibility")? {
        "private" => Ok(Visibility::Private),
        "team" => Ok(Visibility::Team),
        "workspace" => Ok(Visibility::Workspace),
        other => Err(ToolError::BadInput(format!("bad visibility: {other}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// `cells` as a real array decodes as before.
    #[test]
    fn typed_arg_decodes_a_real_array() {
        let cells: Vec<Cell> = typed_arg(&json!([]), "cells").expect("empty array decodes");
        assert!(cells.is_empty());
    }

    /// `cells` as a JSON-ENCODED STRING (the live agent's shape, five turns in a row) decodes to
    /// the same value — the verb's own validators still run on the decoded cells.
    #[test]
    fn typed_arg_tolerates_a_json_encoded_string() {
        let cells: Vec<Cell> = typed_arg(&json!("[]"), "cells").expect("stringified array decodes");
        assert!(cells.is_empty());
    }

    /// A string that is not valid JSON errors with a message that names the right encoding.
    #[test]
    fn typed_arg_steers_on_a_non_json_string() {
        let err = typed_arg::<Vec<Cell>>(&json!("not json"), "cells").unwrap_err();
        let ToolError::BadInput(msg) = err else {
            panic!("expected BadInput")
        };
        assert!(
            msg.contains("JSON-encoded string"),
            "steering message: {msg}"
        );
    }
}
