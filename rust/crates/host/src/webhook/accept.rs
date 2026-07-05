//! The inbound-hit handler — what the gateway route calls AFTER capturing the raw body and the
//! auth result (webhooks scope). The route does the auth (load record → verify per mode → build
//! principal); this module does the **normalize + ingest** half:
//!
//! 1. Build one ingest [`Sample`] over the hook's series (`webhook:{ws}:{id}`), producer
//!    `webhook:{id}`, the raw body as the typed payload, and the standard labels
//!    (`source:webhook`, `method`).
//! 2. Call the existing `ingest_write` path (gated `mcp:ingest.write:call` — the principal built
//!    by the route resolves to it) → the buffer commits + streams it as motion. The webhook
//!    service is a **producer**, not a second store.
//! 3. Drain workspace staging → committed series so the very next `series.latest`/`read` over the
//!    same bridge sees the just-written hit (the round-trip the route must prove).
//! 4. Publish the sample onto its series motion subject (best-effort — state vs motion, rule 3).
//! 5. Update `last_hit_at` on the record (best-effort; failure is non-fatal).
//!
//! Returns the accepted sample (so the route can include the series/seq in its 202 reply).

use lb_auth::Principal;
use lb_ingest::Sample;
use lb_store::{read, write, Store};
use serde_json::{json, Value};

use super::error::WebhookError;
use super::model::{WebhookRecord, TABLE};

/// Accept a verified inbound hit: build the sample, write it through `ingest.write`, drain, publish
/// motion, and bump `last_hit_at`. `principal` is the route-built verified principal (the apikey
/// principal for `bearer` mode, the synthetic `webhook:{id}` principal for `signature` mode); its
/// caps include `mcp:ingest.write:call`. `body` is the **raw received bytes** — preserved verbatim
/// as the sample payload (the contract).
#[allow(clippy::too_many_arguments)]
pub async fn webhook_accept(
    store: &Store,
    bus: &lb_bus::Bus,
    cache: &super::ApiKeyCache,
    pepper: &[u8],
    principal: &Principal,
    ws: &str,
    record: &WebhookRecord,
    body: &[u8],
    method: &str,
    now_secs: u64,
    now_ms: u64,
) -> Result<Sample, WebhookError> {
    let _ = (cache, pepper, ws);

    // The payload is the raw body verbatim, decoded best-effort as JSON (a structured value when
    // the body parses, the raw STRING when it does not — so a non-JSON body is preserved exactly,
    // never dropped or base64-mangled). The label `source:webhook` + the HTTP method ride alongside
    // for downstream filtering / a rule that branches on method.
    let payload = body_to_payload(body);
    let labels = json!({ "source": "webhook", "method": method });

    // The seq is a wall-clock-ms timestamp (monotonic-ish per (series, producer)). Two hits in the
    // same millisecond would dedup (one upserts the other) — a known v1 limit; high-volume callers
    // should use distinct producers or a future per-hook counter (out of v1 scope).
    let sample = Sample {
        series: record.series.clone(),
        producer: WebhookRecord::producer_for(&record.id),
        ts: now_secs,
        seq: now_ms,
        payload,
        labels,
        qos: lb_ingest::Qos::MustDeliver,
    };

    // Write through the existing `ingest_write` path. The principal's caps include
    // `mcp:ingest.write:call`; the verb authorizes + stamps producer + durable-appends to staging.
    // The route-constructed principal already carries the hook's series, so the verb re-stamps the
    // producer to `principal.sub()` — which IS `webhook:{id}` for signature mode and `key:webhook:
    // {id}` for bearer mode (both un-spoofable identities, both fine as the dedup-key second half).
    crate::ingest::ingest_write(store, principal, ws, vec![sample.clone()]).await?;

    // Drain staging → committed series so a same-bridge `series.latest`/`read` sees the hit. The
    // drain is exactly-once per `(series, producer, seq)`, so a write-then-read never double-commits.
    crate::ingest::drain_workspace(store, ws).await?;

    // Publish the motion event (best-effort: state vs motion, rule 3). A live subscriber (a
    // dashboard widget, or the `GET /series/{s}/stream` SSE, or a flow with a trigger on this
    // series) advances without polling.
    let _ = crate::ingest::publish_sample(bus, ws, &sample).await;

    // Bump `last_hit_at` (best-effort — a store error here never fails the accept; the hit is
    // already durable in the series). Read-modify-write preserves the rest of the record.
    let _ = bump_last_hit(store, ws, &record.id, now_secs).await;

    Ok(sample)
}

/// Decode `body` as the sample payload: a structured JSON value when it parses, else the raw bytes
/// as a UTF-8-lossy string (so a non-JSON body is preserved exactly). Never drops the body.
fn body_to_payload(body: &[u8]) -> Value {
    if let Ok(value) = serde_json::from_slice::<Value>(body) {
        return value;
    }
    // A non-JSON body (a form-encoded payload, a raw text) is preserved as a string — never base64-
    // mangled or dropped. (If a future caller needs the raw bytes verbatim, a `bytes`-shaped
    // payload is the upgrade path; v1 keeps it human-readable.)
    Value::String(String::from_utf8_lossy(body).into_owned())
}

/// Read-modify-write `last_hit_at` on the webhook record (best-effort).
async fn bump_last_hit(store: &Store, ws: &str, id: &str, now: u64) -> Result<(), WebhookError> {
    let Some(value) = read(store, ws, TABLE, id).await? else {
        return Ok(());
    };
    let mut record: WebhookRecord = serde_json::from_value(value).map_err(unexpected)?;
    if record.last_hit_at == now {
        return Ok(()); // idempotent
    }
    record.last_hit_at = now;
    let value =
        serde_json::to_value(&record).map_err(|e| lb_store::StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, id, &value).await?;
    Ok(())
}

fn unexpected(e: serde_json::Error) -> WebhookError {
    WebhookError::Store(lb_store::StoreError::Decode(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_body_becomes_structured_payload() {
        let p = body_to_payload(br#"{"event":"push"}"#);
        assert_eq!(p["event"], "push");
    }

    #[test]
    fn non_json_body_becomes_string_payload() {
        let p = body_to_payload(b"hello world");
        assert_eq!(p, Value::String("hello world".into()));
    }

    #[test]
    fn empty_body_becomes_empty_string() {
        let p = body_to_payload(b"");
        assert_eq!(p, Value::String(String::new()));
    }

    #[test]
    fn invalid_utf8_is_lossy_not_dropped() {
        let p = body_to_payload(&[0xff, 0xfe]);
        assert!(p.is_string());
    }
}
