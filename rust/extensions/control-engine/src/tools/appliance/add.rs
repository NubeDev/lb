//! `control-engine.appliance.add` — register (or replace) a `ce_appliance` record. An admin-ish write
//! (control-engine scope: registry writes are distinct from graph writes), gated by its own
//! `mcp:control-engine.appliance.add:call` (self-checked here) AND, host-side, by
//! `store:ce_appliance:write` on the `store.write` callback.
//!
//! Validation (scope): `id`/`node`/`base` are required and non-empty; `base` parses as an http(s)
//! origin; `mode` is `local`|`appliance`. Enrollment of `node` (an `api-keys` `kind="appliance"`
//! machine principal + `edge-trust`) is reused as-is — S4 only RECORDS the id; a future slice may
//! verify it against the api-key store here. Idempotent on `id` (upsert).

use serde_json::{json, Value};

use crate::appliance::record::{Appliance, Mode};
use crate::appliance::store;
use crate::host::{HostCtx, HostError};

/// Run `appliance.add`. Returns `{ id }` on success.
pub async fn run(host: &HostCtx, input: &Value, ts: u64) -> Result<Value, HostError> {
    host.require("control-engine.appliance.add")?;

    let id = req_str(input, "id")?;
    let base = req_str(input, "base")?;
    let name = input
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(id)
        .to_string();
    let mode = parse_mode(input.get("mode").and_then(Value::as_str))?;
    validate_base(base)?;
    // `node` = which LB node owns this appliance. A LOCAL appliance is by definition on THIS node, so
    // the caller need not name it — default to `"local"` (the boot-seed convention) when omitted. A
    // remote `appliance`-mode CE genuinely lives on another node, so its `node` is required (there is
    // no sensible default — the caller must say which node routes to it).
    let node = match input.get("node").and_then(Value::as_str) {
        Some(n) if !n.trim().is_empty() => n,
        _ => match mode {
            Mode::Local => "local",
            Mode::Appliance => {
                return Err(HostError::BadInput(
                    "missing/empty arg: node (required for an appliance-mode CE)".into(),
                ))
            }
        },
    };

    let appliance = Appliance {
        id: id.to_string(),
        name,
        mode,
        node: node.to_string(),
        base: base.to_string(),
        secret_ref: None,
        ts,
    };
    store::put(host, &appliance).await?;
    Ok(json!({ "id": id }))
}

fn parse_mode(s: Option<&str>) -> Result<Mode, HostError> {
    match s {
        Some("local") | None => Ok(Mode::Local),
        Some("appliance") => Ok(Mode::Appliance),
        Some(other) => Err(HostError::BadInput(format!(
            "mode must be local|appliance, got {other}"
        ))),
    }
}

/// A `base` must be a bare or `http(s)://`-prefixed origin (host[:port]); reject anything with a path,
/// query, or an obviously malformed shape. Kept simple — the CE client re-validates at connect.
fn validate_base(base: &str) -> Result<(), HostError> {
    let b = base.trim();
    if b.is_empty() {
        return Err(HostError::BadInput("base is empty".into()));
    }
    let origin = b
        .strip_prefix("http://")
        .or_else(|| b.strip_prefix("https://"))
        .unwrap_or(b);
    // No path segment beyond the origin (a trailing `/` is fine).
    if origin.trim_end_matches('/').contains('/') {
        return Err(HostError::BadInput(format!(
            "base must be an http(s) origin (no path): {base}"
        )));
    }
    if origin.trim_end_matches('/').is_empty() {
        return Err(HostError::BadInput(format!("base has no host: {base}")));
    }
    Ok(())
}

fn req_str<'a>(input: &'a Value, key: &str) -> Result<&'a str, HostError> {
    input
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| HostError::BadInput(format!("missing/empty arg: {key}")))
}
