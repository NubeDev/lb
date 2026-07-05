//! `insight_raise` — the producer WRITE, capability-gated + fully wired (insights umbrella +
//! occurrences + subscriptions + notify scopes).
//!
//! The MCP door for "everything else" (the umbrella's third producer door): an agent, an extension
//! via host-callback, a human via the page/CLI. The rule door (the rhai handle) and the flow door
//! (the `insight` sink node) reach this same verb through the same gate.
//!
//! `producer` is **forced** to the principal's `sub` (host-set, never caller-supplied). The
//! dedup/re-open decision + occurrence append live in `lb_insights::raise` (store-only); THIS layer
//! owns the four host-side effects the crate is deliberately agnostic of:
//!   1. **Tags** — apply `input.tags` to `insight:<id>` through the tag graph (`Source::Producer`).
//!   2. **Bus event** — publish a fire-and-forget `RaiseEvent` on `ws/{ws}/insight/events` (live UI).
//!   3. **Matcher** — evaluate the workspace's subscriptions (`match_subs`) into intents.
//!   4. **Notify** — run each intent through the ladder (`apply_intents`) + post immediate
//!      deliveries (L0/breakthroughs) under each sub's stored principal (fire-time re-checked).

use std::collections::BTreeMap;
use std::sync::Arc;

use lb_auth::Principal;
use lb_insights::{
    apply_intents, match_subs, raise, EventKind, InsightView, RaiseEvent, RaiseInput,
};
use lb_mcp::authorize_tool;

use super::error::InsightSvcError;
use super::notify::{deliver_to_sub, kill_off_owners, load_subs};
use crate::boot::Node;

/// Raise an insight in workspace `ws` as `principal`. See the module doc for the four wired effects.
pub async fn insight_raise(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    mut input: RaiseInput,
) -> Result<lb_insights::RaiseOutcome, InsightSvcError> {
    authorize_tool(principal, ws, "insight.raise").map_err(|_| InsightSvcError::Denied)?;
    input.producer = principal.sub().to_string();
    let now = input.ts;
    let tags = input.tags.clone();

    // The occurrence ring cap comes from the workspace policy (defaults if absent).
    let policy = lb_insights::policy_get(&node.store, ws).await?;

    // 1. The record write + dedup/re-open decision + occurrence append (store-only, in the crate).
    let outcome = raise(&node.store, ws, input, policy.ring_cap).await?;

    // 2. Tags — apply each {k: v} to `insight:<id>` with Producer provenance (the host owns the
    //    tag graph; the crate is tag-agnostic). Best-effort: a tag hiccup must not fail the raise
    //    (the durable record already landed).
    let entity = format!("insight:{}", outcome.id);
    for (k, v) in &tags {
        let tag = lb_tags::Tag::new(k.clone(), serde_json::Value::String(v.clone()));
        let prov = lb_tags::Provenance::new(now, principal.sub(), lb_tags::Source::Producer);
        let _ = crate::tags::tags_add(&node.store, principal, ws, &entity, &tag, &prov).await;
    }

    // 3. Bus event — fire-and-forget live-UI motion on the ws subject (best-effort, §3.3).
    let event = RaiseEvent {
        kind: EventKind::Raise,
        id: outcome.id.clone(),
        dedup_key: outcome.dedup_key.clone(),
        status: outcome.status,
        severity: outcome.severity,
        count: outcome.count,
        ts: now,
    };
    if let Ok(payload) = serde_json::to_vec(&event) {
        // Publish on the workspace-native subject directly (NOT the walled ext bus path).
        let _ = lb_bus::publish(&node.bus, ws, "insight/events", &payload).await;
    }

    // 4. Matcher + notify. Materialize the insight's tag facets for the matcher (the raise input's
    //    tags ARE the facets — no need to re-query the graph for the just-applied set).
    let subs = load_subs(&node.store, ws).await;
    if !subs.is_empty() {
        // The insight's full tag facets from the graph (covers tags applied on PRIOR raises of the
        // same dedup_key, not just this firing's). Falls back to this raise's tags on any error.
        let facets = materialize_facets(node, principal, ws, &entity, &tags).await;
        let origin_ref = origin_ref_of(node, ws, &outcome.id).await;
        let view = InsightView {
            insight_id: &outcome.id,
            dedup_key: &outcome.dedup_key,
            severity: outcome.severity,
            origin_ref: &origin_ref,
            tags: &facets,
            kind: outcome.kind,
        };
        let intents = match_subs(&view, &subs);
        if !intents.is_empty() {
            let acked = matches!(outcome.status, lb_insights::Status::Acked);
            let kill_off = kill_off_owners(&node.store, ws, &subs).await;
            let deliveries = apply_intents(
                &node.store,
                ws,
                &intents,
                acked,
                now,
                &policy,
                &subs,
                &kill_off,
            )
            .await?;
            for d in deliveries {
                if let Some(sub) = subs.iter().find(|s| s.id == d.sub_id) {
                    let body = immediate_body(&d);
                    // Idempotent per (sub, insight, ts) — a re-raise at the same ts upserts.
                    let item_id = format!("insight-post:{}:{}:{}", d.sub_id, d.insight_id, d.ts);
                    deliver_to_sub(node, ws, sub, &item_id, &body, now).await;
                }
            }
        }
    }

    Ok(outcome)
}

/// Materialize the insight's tag facets as `{ k: v }` for the matcher's subset check. Reads the tag
/// graph (`tags.of`, stringifying each value); on any error falls back to this raise's declared
/// tags so a tag-graph hiccup can't silently drop a match.
async fn materialize_facets(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    entity: &str,
    fallback: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    match crate::tags::tags_of(&node.store, principal, ws, entity).await {
        Ok(applied) if !applied.is_empty() => applied
            .into_iter()
            .map(|a| {
                let v = match a.value {
                    serde_json::Value::String(s) => s,
                    other => other.to_string(),
                };
                (a.key, v)
            })
            .collect(),
        _ => fallback.clone(),
    }
}

/// Read the origin ref off the just-written insight (the matcher's `origin_ref` axis). Cheap single
/// read; keeps the raise outcome lean (origin isn't echoed there).
async fn origin_ref_of(node: &Arc<Node>, ws: &str, id: &str) -> String {
    match lb_insights::get(&node.store, ws, id).await {
        Ok(Some(insight)) => insight.origin.reference,
        _ => String::new(),
    }
}

/// The one-line body for an immediate (L0 / breakthrough) delivery — mechanical v1 text + the key
/// (a rich `render:` table is the named follow-up, notify scope).
fn immediate_body(d: &lb_insights::Delivery) -> String {
    format!(
        "insight {} — {} ({:?}) [view]",
        d.dedup_key, d.insight_id, d.severity
    )
}
