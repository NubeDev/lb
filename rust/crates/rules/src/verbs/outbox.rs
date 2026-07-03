//! The `outbox.*` rhai handle — `outbox.enqueue(#{id, target, action, payload})` and
//! `outbox.status(id)` (rules-messaging-scope). A rule **stages** a must-deliver effect and may
//! **inspect** the queue's status; it does NOT drain or adjudicate it — the relay-driver verbs
//! (`due`/`mark_delivered`/`mark_failed`) are the sidecar's surface and are deliberately absent from
//! this handle (Resolved decisions), so a rule can never race the real relay.
//!
//! Routed through the ONE MCP contract via [`MessagingSeam`]: each call re-runs the host's workspace
//! pin + `caps::check`. `enqueue` is a motion-producing write (charged against the shared per-run
//! [`WriteMeter`]); `status` is a read (uncharged). Deterministic ids (`now` + counter) → a re-run
//! upserts.

use std::sync::Arc;

use rhai::{Dynamic, Engine, EvalAltResult, Map};
use serde_json::{json, Value};

use crate::grid::{json_to_dynamic, rhai_err};
use crate::meter::WriteMeter;
use crate::seam::MessagingSeam;
use crate::verbs::inbox::{map_str, seam_err};

/// The `outbox` scope value — the messaging seam + the shared write meter + the run's logical clock.
#[derive(Clone)]
pub struct OutboxHandle {
    seam: Arc<dyn MessagingSeam>,
    meter: Arc<WriteMeter>,
    now: u64,
}

impl OutboxHandle {
    pub fn new(seam: Arc<dyn MessagingSeam>, meter: Arc<WriteMeter>, now: u64) -> Self {
        Self { seam, meter, now }
    }

    /// outbox.enqueue(#{id, target, action, payload}) → () — stage a must-deliver effect. Charged.
    /// `id` is author-supplied for idempotency; omitted → a deterministic per-run id (`now` + counter).
    pub fn enqueue(&self, effect: Map) -> Result<(), Box<EvalAltResult>> {
        let target = map_str(&effect, "target")
            .ok_or_else(|| rhai_err("outbox.enqueue: missing `target`"))?;
        let action = map_str(&effect, "action")
            .ok_or_else(|| rhai_err("outbox.enqueue: missing `action`"))?;
        let payload = effect
            .get("payload")
            .map(|v| crate::grid::dynamic_to_json(v))
            .unwrap_or(Value::Null);
        let seq = self.meter.charge().map_err(rhai_err)?;
        let id =
            map_str(&effect, "id").unwrap_or_else(|| format!("rule-outbox-{}-{seq}", self.now));
        self.call(
            "outbox.enqueue",
            json!({ "id": id, "target": target, "action": action,
                    "payload": payload, "ts": self.now }),
        )?;
        Ok(())
    }

    /// outbox.status(id) → the effect's status entry, or the whole workspace status when `id` is empty.
    /// A read (uncharged). The underlying `outbox.status` verb returns the workspace's pending/failed
    /// summary; the rule filters by id if it named one.
    pub fn status(&self, id: &str) -> Result<Dynamic, Box<EvalAltResult>> {
        let out = self.call("outbox.status", json!({}))?;
        if id.is_empty() {
            return Ok(json_to_dynamic(&out));
        }
        // Return the matching effect's entry if the status surfaces per-effect rows; else the summary.
        if let Some(effects) = out.get("effects").and_then(|e| e.as_array()) {
            if let Some(hit) = effects
                .iter()
                .find(|e| e.get("id").and_then(|v| v.as_str()) == Some(id))
            {
                return Ok(json_to_dynamic(hit));
            }
        }
        Ok(json_to_dynamic(&out))
    }

    fn call(&self, tool: &str, input: Value) -> Result<Value, Box<EvalAltResult>> {
        self.seam.call(tool, input).map_err(seam_err)
    }
}

pub fn register(engine: &mut Engine) {
    engine.register_type_with_name::<OutboxHandle>("Outbox");
    engine.register_fn("enqueue", |h: &mut OutboxHandle, e: Map| h.enqueue(e));
    engine.register_fn("status", |h: &mut OutboxHandle, id: &str| h.status(id));
    engine.register_fn("status", |h: &mut OutboxHandle| h.status(""));
}
