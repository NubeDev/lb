//! The `channel.*` rhai handle — `channel.post(cid, #{…})`, `channel.history(cid, n)`,
//! `channel.edit(cid, mid, #{…})`, `channel.delete(cid, mid)`, `channel.list()`
//! (rules-messaging-scope). The full channel surface a rule needs to post to, read, and amend a
//! channel — routed through the ONE MCP contract via [`MessagingSeam`], so each call re-runs the
//! host's workspace pin + the channel `bus:chan/{cid}:{Pub|Sub}` gate under `caller ∩ grant`. A deny
//! is opaque; `history`/`list` are reads (uncharged), `post`/`edit`/`delete` are motion-producing
//! writes (charged against the shared per-run [`WriteMeter`]).
//!
//! **The worker-kind fence lives here, not in the generic `channel.post` MCP verb** (Resolved
//! decisions): the verb keeps full parity with the WS path (a `kind:"query"`/`kind:"agent"` post
//! spawns the inline/background worker), but a *rule* must not quietly kick an unbounded agent/query
//! run from a bounded, synchronous body — so [`ChannelHandle::post`] rejects those kinds with an
//! author error ("a rule cannot spawn a run — use a flow") before the write ever reaches the seam.

use std::sync::Arc;

use rhai::{Dynamic, Engine, EvalAltResult, Map};
use serde_json::{json, Value};

use crate::grid::{json_to_dynamic, rhai_err};
use crate::meter::WriteMeter;
use crate::seam::MessagingSeam;
use crate::verbs::inbox::{map_str, map_to_json, seam_err};

/// The `channel` scope value — the messaging seam + the shared write meter + the run's logical clock
/// (feeds deterministic post ids).
#[derive(Clone)]
pub struct ChannelHandle {
    seam: Arc<dyn MessagingSeam>,
    meter: Arc<WriteMeter>,
    now: u64,
}

impl ChannelHandle {
    pub fn new(seam: Arc<dyn MessagingSeam>, meter: Arc<WriteMeter>, now: u64) -> Self {
        Self { seam, meter, now }
    }

    /// channel.post(cid, #{kind?, body, id?}) → the stored item. Charged (a write). The author is
    /// forced to the caller by the MCP verb (never spoofable). `id` is author-supplied for idempotency;
    /// omitted → a deterministic per-run id (`now` + counter) so a re-run upserts.
    ///
    /// **Fenced:** a `kind:"agent"`/`kind:"query"` post is rejected with an author error — a rule is
    /// bounded and synchronous and must not spawn a run (Resolved decisions). `kind:"text"` (or no
    /// kind) is a plain chat post and passes; the write meter is only charged AFTER the fence passes.
    pub fn post(&self, cid: &str, item: Map) -> Result<Dynamic, Box<EvalAltResult>> {
        let body = self.fenced_body(&item)?;
        let seq = self.meter.charge().map_err(rhai_err)?;
        let id = map_str(&item, "id").unwrap_or_else(|| format!("rule-channel-{}-{seq}", self.now));
        let out = self.call(
            "channel.post",
            json!({ "cid": cid, "id": id, "body": body, "ts": self.now }),
        )?;
        Ok(json_to_dynamic(&out))
    }

    /// channel.history(cid, n) → the last `n` items (a bounded snapshot, not a watch). A read
    /// (uncharged). `n <= 0` returns the whole history.
    pub fn history(&self, cid: &str, n: i64) -> Result<Dynamic, Box<EvalAltResult>> {
        let mut input = json!({ "cid": cid });
        if n > 0 {
            input["n"] = json!(n as u64);
        }
        let out = self.call("channel.history", input)?;
        Ok(json_to_dynamic(out.get("messages").unwrap_or(&Value::Null)))
    }

    /// channel.edit(cid, mid, #{body}) → the updated item. Charged (a write). Idempotent on `mid`.
    pub fn edit(&self, cid: &str, mid: &str, patch: Map) -> Result<Dynamic, Box<EvalAltResult>> {
        let body = map_str(&patch, "body").unwrap_or_default();
        let _seq = self.meter.charge().map_err(rhai_err)?;
        let out = self.call(
            "channel.edit",
            json!({ "cid": cid, "id": mid, "body": body, "ts": self.now }),
        )?;
        Ok(json_to_dynamic(&out))
    }

    /// channel.delete(cid, mid) → () — remove an item. Charged (a write). Idempotent on `mid`.
    pub fn delete(&self, cid: &str, mid: &str) -> Result<(), Box<EvalAltResult>> {
        let _seq = self.meter.charge().map_err(rhai_err)?;
        self.call("channel.delete", json!({ "cid": cid, "id": mid }))?;
        Ok(())
    }

    /// channel.list() → the workspace's channels. A read (uncharged).
    pub fn list(&self) -> Result<Dynamic, Box<EvalAltResult>> {
        let out = self.call("channel.list", json!({}))?;
        Ok(json_to_dynamic(out.get("channels").unwrap_or(&Value::Null)))
    }

    /// Resolve the post body, enforcing the worker-kind fence. A `kind:"agent"`/`kind:"query"` item
    /// is rejected (a rule cannot spawn a run); `kind:"text"` / no kind is a plain chat body (the raw
    /// `body` string, exactly as the WS path sends a chat message); any other worker-facing kind rides
    /// as a JSON envelope inside `body` (parity with the WS payload path) — none of which are workers.
    fn fenced_body(&self, item: &Map) -> Result<String, Box<EvalAltResult>> {
        let kind = map_str(item, "kind");
        match kind.as_deref() {
            Some("agent") | Some("query") => Err(rhai_err(
                "channel.post: a rule cannot spawn a run — use a flow (kind agent/query rejected)",
            )),
            None | Some("text") => Ok(map_str(item, "body").unwrap_or_default()),
            // A non-worker kind-tagged payload rides as JSON inside `body` (WS payload parity).
            Some(_) => {
                Ok(serde_json::to_string(&map_to_json(item))
                    .map_err(|e| rhai_err(e.to_string()))?)
            }
        }
    }

    fn call(&self, tool: &str, input: Value) -> Result<Value, Box<EvalAltResult>> {
        self.seam.call(tool, input).map_err(seam_err)
    }
}

pub fn register(engine: &mut Engine) {
    engine.register_type_with_name::<ChannelHandle>("Channel");
    engine.register_fn("post", |h: &mut ChannelHandle, cid: &str, item: Map| {
        h.post(cid, item)
    });
    engine.register_fn("history", |h: &mut ChannelHandle, cid: &str, n: i64| {
        h.history(cid, n)
    });
    engine.register_fn("history", |h: &mut ChannelHandle, cid: &str| {
        h.history(cid, 0)
    });
    engine.register_fn(
        "edit",
        |h: &mut ChannelHandle, cid: &str, mid: &str, patch: Map| h.edit(cid, mid, patch),
    );
    engine.register_fn("delete", |h: &mut ChannelHandle, cid: &str, mid: &str| {
        h.delete(cid, mid)
    });
    engine.register_fn("list", |h: &mut ChannelHandle| h.list());
}
