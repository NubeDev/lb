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
    // Low-level guard: `insight.ts` is stored + rendered as epoch MILLISECONDS (the UI does
    // `new Date(ts)` / `Date.now() - ts`, `insight::reactor` stamps `as_millis`). Two producer
    // doors get it wrong and land records in 1970:
    //   1. A door that forgot to stamp `ts` (or handed `0`) → the Unix epoch.
    //   2. The gateway `rules/run` route stamps `gw.now()`, which is epoch SECONDS (`as_secs()`) —
    //      `new Date(1.78e9)` renders as Jan 1970, and `Date.now() - 1.78e9` reads "20623d ago".
    // This host layer — the single funnel every producer door reaches — normalizes both to millis
    // (the crate stays wall-clock-free, testing §3). `0` backfills the wall-clock. A value in the
    // plausible epoch-SECONDS band (~2001..33658, i.e. `[1e9, 1e12)`) is scaled ×1000. A real
    // epoch-millis `ts` (≥ 1e12) and a tiny deterministic test/logical clock (< 1e9) both pass
    // through untouched — so flows + tests seeding fixed small clocks stay reproducible.
    input.ts = normalize_ts(input.ts);
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

/// Epoch-seconds band: any real wall-clock date from ~2001-09 (`1e9`) up to ~year 33658 (`1e12`).
/// A `ts` here was almost certainly stamped in SECONDS (e.g. the gateway's `gw.now()` = `as_secs()`);
/// scale it to the millis `insight.ts` is defined in. Below `1e9` = a tiny logical/test clock (leave
/// it); at/above `1e12` = already epoch-millis (leave it).
pub(super) const TS_SECONDS_MIN: u64 = 1_000_000_000; // 1e9  — ~2001-09-09 in seconds
pub(super) const TS_MILLIS_MIN: u64 = 1_000_000_000_000; // 1e12 — ~2001-09-09 in millis

/// Normalize a raise `ts` to epoch milliseconds. `0` ⇒ the host wall-clock. A value in the
/// epoch-seconds band (`[1e9, 1e12)`) ⇒ ×1000. Everything else (a real ms clock, or a tiny
/// deterministic test/logical clock) passes through unchanged. See the guard in [`insight_raise`].
fn normalize_ts(ts: u64) -> u64 {
    if ts == 0 {
        now_ms()
    } else if (TS_SECONDS_MIN..TS_MILLIS_MIN).contains(&ts) {
        ts * 1000
    } else {
        ts
    }
}

/// The host wall-clock as epoch milliseconds — the unit `insight.ts` is stored + rendered in
/// (`InsightsList.timeAgo` does `Date.now() - ts`, `insight::reactor` stamps `as_millis`). Used to
/// backfill a `ts` a producer door omitted (see the guard in [`insight_raise`]).
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
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
