//! The `inbox.*` rhai handle — `inbox.list(channel)`, `inbox.record(#{channel, id, body})`,
//! `inbox.resolve(item_id, decision)` (rules-messaging-scope), and
//! `inbox.request_approval(#{id, channel, body, route, on_approve})` (rules-approvals-scope): raise a
//! `needs:approval` item that stages a **held** gated effect the approval reactor releases on approval.
//! The full attention-item surface a rule needs to raise, read, resolve, and gate items — routed
//! through the ONE MCP contract via [`MessagingSeam`],
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
    /// `item_id` (re-resolving upserts, last decision wins). `decision` is one of `"approved"`,
    /// `"rejected"`, `"deferred"` (the reviewer's verdict — the `inbox.resolve` verb's `Decision` enum);
    /// anything else is rejected with an author error listing the valid verdicts.
    pub fn resolve(&self, item_id: &str, decision: &str) -> Result<(), Box<EvalAltResult>> {
        match decision {
            "approved" | "rejected" | "deferred" => {}
            other => {
                return Err(rhai_err(format!(
                    "inbox.resolve: decision must be \"approved\", \"rejected\", or \"deferred\" (got {other:?})"
                )))
            }
        }
        let _seq = self.meter.charge().map_err(rhai_err)?;
        self.call(
            "inbox.resolve",
            json!({ "item_id": item_id, "decision": decision, "ts": self.now }),
        )?;
        Ok(())
    }

    /// inbox.request_approval(#{ id, channel, body, route, on_approve }) → the approval item id.
    /// Raises a `needs:approval` item addressed to `route` AND durably stages the `on_approve` effect
    /// it should fire IF approved — the effect is enqueued **`held`** (the relay skips it) and released
    /// to the outbox only when the item resolves `Approved` (rules-approvals scope). Two writes, both
    /// charged against the shared per-run [`WriteMeter`].
    ///
    /// **Compound-write order (partial-failure contract):** the held effect is staged FIRST, then the
    /// item is recorded — so a `needs:approval` item never exists without its gated effect. A mid-verb
    /// fault (e.g. the item write is denied) leaves at most the effect staged, which is harmless: it is
    /// held, never delivered, and GC-able. Both writes route through the ONE MCP contract under
    /// `caller ∩ grant` (`outbox.enqueue` cap for the effect, `inbox.record` cap for the item); a deny
    /// on either step is opaque.
    ///
    /// `on_approve` is `#{ target, action, payload }` — a normal outbox effect. `route` (e.g.
    /// `"team:managers"`) is advisory addressing folded into the item body's tag.
    pub fn request_approval(&self, req: Map) -> Result<String, Box<EvalAltResult>> {
        let channel = map_str(&req, "channel")
            .ok_or_else(|| rhai_err("inbox.request_approval: missing `channel`"))?;
        let body = map_str(&req, "body").unwrap_or_default();
        let route = map_str(&req, "route").unwrap_or_default();
        let on_approve = req
            .get("on_approve")
            .and_then(|v| v.clone().try_cast::<Map>())
            .ok_or_else(|| {
                rhai_err(
                    "inbox.request_approval: missing `on_approve` #{ target, action, payload }",
                )
            })?;
        let target = map_str(&on_approve, "target")
            .ok_or_else(|| rhai_err("inbox.request_approval: `on_approve` missing `target`"))?;
        let action = map_str(&on_approve, "action")
            .ok_or_else(|| rhai_err("inbox.request_approval: `on_approve` missing `action`"))?;
        let payload = on_approve
            .get("payload")
            .map(crate::grid::dynamic_to_json)
            .unwrap_or(Value::Null);

        // The `needs:approval` tag + route ride the existing body-tag convention (v1 — no Item schema
        // change), so the same reviewer UI / reactor parse it as they do the coding-workflow's.
        let tagged_body = if route.is_empty() {
            format!("needs:approval {body}")
        } else {
            format!("needs:approval route:{route} {body}")
        };

        // Stage the HELD effect FIRST (partial-failure contract), then record the item. Both charged.
        let seq = self.meter.charge().map_err(rhai_err)?;
        let item_id = map_str(&req, "id").unwrap_or_else(|| self.derived_id("approval", seq));
        self.call(
            "outbox.enqueue_held",
            json!({ "item_id": item_id, "target": target, "action": action,
                    "payload": payload, "ts": self.now }),
        )?;
        let _seq2 = self.meter.charge().map_err(rhai_err)?;
        self.call(
            "inbox.record",
            json!({ "channel": channel, "id": item_id, "body": tagged_body, "ts": self.now }),
        )?;
        Ok(item_id)
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
    engine.register_fn("resolve", |h: &mut InboxHandle, id: &str, d: &str| {
        h.resolve(id, d)
    });
    engine.register_fn("request_approval", |h: &mut InboxHandle, req: Map| {
        h.request_approval(req)
    });
}
