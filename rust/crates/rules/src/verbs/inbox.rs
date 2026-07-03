//! The `inbox.*` rhai handle — `inbox.list(channel)`, `inbox.record(#{channel, id, body})`,
//! `inbox.resolve(item_id, decision)` (rules-messaging-scope). The full attention-item surface a rule
//! needs to raise, read, and resolve items — routed through the ONE MCP contract via [`MessagingSeam`],
//! so each call re-runs the host's workspace pin + `caps::check` under `caller ∩ grant`. A deny is
//! opaque (a rhai error the rule can catch but not distinguish from "empty"); reads are uncharged, the
//! two writes (`record`, `resolve`) are charged against the shared per-run [`WriteMeter`].

use std::sync::Arc;

use rhai::{Dynamic, Engine, EvalAltResult, Map};
use serde_json::{json, Value};

use crate::grid::{dynamic_to_json, json_to_dynamic, rhai_err};
use crate::meter::WriteMeter;
use crate::seam::{MessagingSeam, SeamError};

/// The `inbox` scope value — closes over the messaging seam, the shared write meter, and the run's
/// logical clock (for deterministic ids).
#[derive(Clone)]
pub struct InboxHandle {
    seam: Arc<dyn MessagingSeam>,
    meter: Arc<WriteMeter>,
    now: u64,
}

impl InboxHandle {
    pub fn new(seam: Arc<dyn MessagingSeam>, meter: Arc<WriteMeter>, now: u64) -> Self {
        Self { seam, meter, now }
    }

    /// inbox.list(channel) → array of items. A workspace-scoped read; uncharged by the write meter.
    pub fn list(&self, channel: &str) -> Result<Dynamic, Box<EvalAltResult>> {
        let out = self.call("inbox.list", json!({ "channel": channel }))?;
        Ok(json_to_dynamic(out.get("items").unwrap_or(&Value::Null)))
    }

    /// inbox.record(#{channel, id, body}) → () — raise an attention item. Charged (a write). `id` is
    /// author-supplied for idempotency; if omitted a deterministic per-run id is derived (`now` +
    /// counter) so a re-run upserts rather than duplicating.
    pub fn record(&self, item: Map) -> Result<(), Box<EvalAltResult>> {
        let channel =
            map_str(&item, "channel").ok_or_else(|| rhai_err("inbox.record: missing `channel`"))?;
        let body = map_str(&item, "body").unwrap_or_default();
        let seq = self.meter.charge().map_err(rhai_err)?;
        let id = map_str(&item, "id").unwrap_or_else(|| self.derived_id("inbox", seq));
        self.call(
            "inbox.record",
            json!({ "channel": channel, "id": id, "body": body, "ts": self.now }),
        )?;
        Ok(())
    }

    /// inbox.resolve(item_id, decision) → () — close an item. Charged (a write). Idempotent on
    /// `item_id` (re-resolving upserts, last decision wins).
    pub fn resolve(&self, item_id: &str, decision: Map) -> Result<(), Box<EvalAltResult>> {
        let decision = map_to_json(&decision);
        let _seq = self.meter.charge().map_err(rhai_err)?;
        self.call(
            "inbox.resolve",
            json!({ "item_id": item_id, "decision": decision, "ts": self.now }),
        )?;
        Ok(())
    }

    /// A deterministic id from the run's logical clock + the write's ordinal (no wall-clock/random).
    fn derived_id(&self, kind: &str, seq: u32) -> String {
        format!("rule-{kind}-{}-{seq}", self.now)
    }

    fn call(&self, tool: &str, input: Value) -> Result<Value, Box<EvalAltResult>> {
        self.seam.call(tool, input).map_err(seam_err)
    }
}

/// Map a [`SeamError`] to a rhai error. `Denied` is OPAQUE — no plane/cap detail leaks; `Failed` is
/// author feedback, surfaced verbatim.
pub fn seam_err(e: SeamError) -> Box<EvalAltResult> {
    match e {
        SeamError::Denied => rhai_err("denied"),
        SeamError::Failed(m) => rhai_err(m),
    }
}

/// Read a string field from a rhai map.
pub fn map_str(m: &Map, key: &str) -> Option<String> {
    m.get(key).and_then(|v| v.clone().into_string().ok())
}

/// Convert a rhai map to a JSON object.
pub fn map_to_json(m: &Map) -> Value {
    let mut obj = serde_json::Map::new();
    for (k, v) in m.iter() {
        obj.insert(k.to_string(), dynamic_to_json(v));
    }
    Value::Object(obj)
}

pub fn register(engine: &mut Engine) {
    engine.register_type_with_name::<InboxHandle>("Inbox");
    engine.register_fn("list", |h: &mut InboxHandle, channel: &str| h.list(channel));
    engine.register_fn("record", |h: &mut InboxHandle, item: Map| h.record(item));
    engine.register_fn("resolve", |h: &mut InboxHandle, id: &str, d: Map| {
        h.resolve(id, d)
    });
}
