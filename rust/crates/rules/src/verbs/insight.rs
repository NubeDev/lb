//! The `insight.*` rhai handle — `insight.raise(#{…}) -> id`, `insight.ack(id)`,
//! `insight.close(id [, note])` (rule-raises-insight-scope). The **rule producer door** onto the
//! insight plane: a threshold rule notices a fault and records it inline, without wiring a flow +
//! `insight` sink node. Each method is one `MessagingSeam::call("insight.<verb>", …)` over the SAME
//! generic chokepoint the `inbox`/`outbox`/`channel` handles ride — so every call re-runs the host's
//! workspace pin + `mcp:insight.<verb>:call` gate under `caller ∩ grant`, a deny is opaque, and the
//! `producer`/`acked_by`/`resolved_by` are host-forced from the principal (un-spoofable).
//!
//! **`close` maps to the `insight.resolve` verb** — "close" is the author-facing lifecycle-end name a
//! rule author means; the underlying MCP verb + capability are `insight.resolve` / `mcp:insight.resolve:call`
//! (a reader grepping for `resolve` should know `close` is its cage name, and vice-versa). No new MCP
//! verb, no new capability — this is a cage-side door onto the three EXISTING producer/lifecycle verbs.
//!
//! **Metering:** `raise`/`ack`/`close` are motion-producing writes — each charged against the shared
//! per-run [`WriteMeter`], AFTER validation and AFTER the `route:false` short-circuit (mirroring
//! `ChannelHandle::post`, which charges after its fence). An insight-storm loop trips `max_writes`.
//!
//! **`route:false` (read-only panel run):** an `insight.raise` is a STRONGER effect than `alert()` —
//! it writes a durable record AND fans out the notify ladder. If `alert()` is suppressed on a panel
//! repaint (rules-for-widgets slice 2), raising an insight must be too. So on a `route:false` run each
//! method is a **no-op**: it charges nothing, logs an honest skip line, and returns an echoed/synthetic
//! id (`raise`) or `()` (`ack`/`close`). Dedup does not save us — every repaint would still bump
//! `count`, append an occurrence row, and re-fire the matcher (rule-raises-insight-scope §route:false).

use std::sync::Arc;

use rhai::{Engine, EvalAltResult, Map};
use serde_json::{json, Value};

use crate::grid::rhai_err;
use crate::meter::WriteMeter;
use crate::seam::MessagingSeam;
use crate::verbs::emit::Collectors;
use crate::verbs::inbox::{map_str, map_to_json, seam_err};

/// The `insight` scope value — the messaging seam + the shared write meter + the run's logical clock
/// + the run's `route` flag + the run's origin ref (the rule's id/name, stamped into `Origin.ref`).
/// The collectors ride along so a `route:false` skip is a visible cage log line, not a silent drop.
#[derive(Clone)]
pub struct InsightHandle {
    seam: Arc<dyn MessagingSeam>,
    meter: Arc<WriteMeter>,
    now: u64,
    /// `false` = a read-only panel repaint: raise/ack/close are no-ops (charge nothing, log the skip).
    route: bool,
    /// The producer ref stamped into `Origin.ref` when the author omits `origin` — the rule's id/name
    /// (the cage's provenance; the host still force-stamps `producer` from the principal).
    origin_ref: String,
    /// The run's log sink — a `route:false` skip records an honest info line here (visible, not silent).
    collectors: Arc<Collectors>,
}

impl InsightHandle {
    pub fn new(
        seam: Arc<dyn MessagingSeam>,
        meter: Arc<WriteMeter>,
        now: u64,
        route: bool,
        origin_ref: String,
        collectors: Arc<Collectors>,
    ) -> Self {
        Self {
            seam,
            meter,
            now,
            route,
            origin_ref,
            collectors,
        }
    }

    /// insight.raise(#{ dedup_key, severity, title, body?, tags?, origin?, occurrence? }) -> String.
    /// Reaches the EXISTING `insight.raise` verb: charge the meter (after validation + the `route:false`
    /// short-circuit), inject `ts: now`, default `origin` to `{ kind:"rule", ref:<run's rule id> }` when
    /// omitted, return the outcome `id` so the author can `ack`/`close` it later in the same body.
    ///
    /// The map's `producer` (and the verb's `acked_by`/`resolved_by`) are host-forced from the principal
    /// — a rule cannot forge an actor even by putting one in the map (the host overwrites it).
    pub fn raise(&self, item: Map) -> Result<String, Box<EvalAltResult>> {
        // Validate the required fields up front (BadInput author feedback, not an opaque deny).
        let dedup_key = map_str(&item, "dedup_key")
            .ok_or_else(|| rhai_err("insight.raise: missing `dedup_key`"))?;
        if map_str(&item, "severity").is_none() {
            return Err(rhai_err(
                "insight.raise: missing `severity` (info/warning/critical)",
            ));
        }
        if map_str(&item, "title").is_none() {
            return Err(rhai_err("insight.raise: missing `title`"));
        }

        // `route:false` short-circuit — no write, no charge, an honest skip line, an echoed id.
        if !self.route {
            self.log_skip("raise");
            return Ok(format!("skipped:{dedup_key}"));
        }

        let _seq = self.meter.charge().map_err(rhai_err)?;
        let mut input = map_to_json(&item);
        // The cage owns `ts` (the run's logical clock — no wall-clock, testing §3) and defaults the
        // `origin` to the run's provenance when the author omits it (the rule IS the origin).
        if let Value::Object(obj) = &mut input {
            obj.insert("ts".into(), json!(self.now));
            obj.entry("origin")
                .or_insert_with(|| json!({ "kind": "rule", "ref": self.origin_ref }));
        }
        let out = self.call("insight.raise", input)?;
        // The verb echoes the outcome `id`; surface it so the author can ack/close it later.
        out.get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| rhai_err("insight.raise: verb returned no id"))
    }

    /// insight.ack(id) -> () — `open → acked` over the EXISTING `insight.ack` verb. Charged (a write);
    /// idempotent on `id`. `acked_by` is host-forced from the principal. `route:false` = no-op.
    pub fn ack(&self, id: &str) -> Result<(), Box<EvalAltResult>> {
        if !self.route {
            self.log_skip("ack");
            return Ok(());
        }
        let _seq = self.meter.charge().map_err(rhai_err)?;
        self.call("insight.ack", json!({ "id": id, "ts": self.now }))?;
        Ok(())
    }

    /// insight.close(id [, note]) -> () — `* → resolved` over the EXISTING `insight.resolve` verb
    /// ("close" is the author-facing name for `resolve`). Charged (a write); idempotent on `id`.
    /// `resolved_by` is host-forced from the principal. `route:false` = no-op.
    pub fn close(&self, id: &str, note: Option<&str>) -> Result<(), Box<EvalAltResult>> {
        if !self.route {
            self.log_skip("close");
            return Ok(());
        }
        let _seq = self.meter.charge().map_err(rhai_err)?;
        let mut input = json!({ "id": id, "ts": self.now });
        if let (Value::Object(obj), Some(n)) = (&mut input, note) {
            obj.insert("note".into(), json!(n));
        }
        self.call("insight.resolve", input)?;
        Ok(())
    }

    /// Record an honest "skipped on a read-only run" info line so the author isn't confused by a missing
    /// record (the same honesty rule the workbench applies to a suppressed `alert()`).
    fn log_skip(&self, verb: &str) {
        self.collectors.log(
            "info",
            format!("insight.{verb} skipped: read-only panel run (route:false)"),
        );
    }

    fn call(&self, tool: &str, input: Value) -> Result<Value, Box<EvalAltResult>> {
        self.seam.call(tool, input).map_err(seam_err)
    }
}

pub fn register(engine: &mut Engine) {
    engine.register_type_with_name::<InsightHandle>("Insight");
    engine.register_fn("raise", |h: &mut InsightHandle, item: Map| h.raise(item));
    engine.register_fn("ack", |h: &mut InsightHandle, id: &str| h.ack(id));
    engine.register_fn("close", |h: &mut InsightHandle, id: &str| h.close(id, None));
    engine.register_fn("close", |h: &mut InsightHandle, id: &str, note: &str| {
        h.close(id, Some(note))
    });
}
