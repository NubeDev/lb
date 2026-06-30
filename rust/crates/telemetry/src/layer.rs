//! `SurrealCappedLayer` — the `tracing-subscriber` **Layer** that writes each `lb.telemetry` event
//! to the FIFO-capped `telemetry` table via [`lb_store::capped_insert`] (telemetry-console scope).
//! It is a **peer** to the OTLP exporter and the stderr layer — the `node` binary selects which
//! layers to install from config (symmetric nodes, rule #1); none is privileged in the emit path.
//!
//! ## Correctness posture
//!
//! - **Fire-and-forget store write.** Each `on_event` spawns a `capped_insert` task and returns
//!   immediately — telemetry must NEVER block the instrumented hot path (a sick store cannot stall
//!   the host). A spawn failure is swallowed (the ring is recent history, not a durability record).
//! - **Head-ratio sampling.** A counter-based sampler keeps every Nth event (cheap, deterministic —
//!   no RNG on the hot path). A flood cannot thrash the ring; the cap holds the bound regardless.
//! - **FIFO key = configurable selector.** [`KeySelector::PerSource`] caps the newest N **per
//!   source** (a chatty source cannot evict a quiet one); [`KeySelector::Global`] caps the newest N
//!   per workspace across sources (the backstop). Both from the same `capped_insert`.
//! - **Redaction by construction.** The Layer writes ONLY the fields the emitter attached — which
//!   are already the digested/redacted schema (the `Secret<T>` type cannot reach here). It adds zero
//!   new opportunity to capture a secret.
//!
//! The trim is the single-transaction primitive ([`lb_store::capped_insert`]); the strictest trim
//! cadence (every insert) is used for v1 so the table NEVER exceeds the cap. Amortized trim (every m
//! inserts, a documented bounded slack) is a deferred optimization — the design holds the hard
//! invariant first.

use std::sync::atomic::{AtomicU64, Ordering};

use lb_bus::Bus;
use lb_store::{capped_insert, new_ulid, Store};
use serde_json::Value;
use tracing::field::Visit;
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

use crate::record::TABLE;
use crate::subject::TAIL_SUBJECT;

/// The default per-key cap (telemetry-console scope open question): ≈1000/source on a workstation.
/// Smaller on a constrained appliance, larger on a hub — both are config overrides, never code.
pub const DEFAULT_CAP: usize = 1000;

/// The default head-ratio sampling: keep every Nth event (1 = keep all). Cheap, deterministic, no RNG
/// on the hot path. Honors the observability scope's "head-ratio sampling so a flood can't thrash."
pub const DEFAULT_SAMPLE_EVERY: u64 = 1;

/// The FIFO key selector — the configurable "newest N per WHAT" (telemetry-console scope). The same
/// `capped_insert` helper serves both; the choice is the caller's.
#[derive(Debug, Clone, Copy)]
pub enum KeySelector {
    /// Newest `cap` per **source** (the default): a chatty extension/tool cannot evict a quiet one.
    PerSource,
    /// Newest `cap` per **workspace** across all sources (the backstop): the ring is bounded globally.
    Global,
}

/// A `tracing-subscriber` Layer that writes each `lb.telemetry` event into the capped `telemetry`
/// table. Clone the store handle and install alongside any other layers. When a [`Bus`] is attached,
/// each written row is ALSO published to the ws-walled tail subject so `telemetry.tail` can stream it
/// live (the tail is motion; the capped row is the recent-history record — §3.3).
pub struct SurrealCappedLayer {
    store: Store,
    bus: Option<Bus>,
    cap: usize,
    sample_every: u64,
    counter: AtomicU64,
    key: KeySelector,
}

impl SurrealCappedLayer {
    /// Build a layer over `store` with the default cap (1000/key) and no sampling. The store handle
    /// is cloned (cheap) per write.
    pub fn new(store: Store) -> Self {
        Self {
            store,
            bus: None,
            cap: DEFAULT_CAP,
            sample_every: DEFAULT_SAMPLE_EVERY,
            counter: AtomicU64::new(0),
            key: KeySelector::PerSource,
        }
    }

    /// Attach the bus so each written row is also mirrored onto the ws-walled tail subject for
    /// `telemetry.tail`. Without this the ring still fills (query/trace work); only the live tail is
    /// absent.
    pub fn with_bus(mut self, bus: Bus) -> Self {
        self.bus = Some(bus);
        self
    }

    /// Override the per-key cap (smaller on an appliance, larger on a hub).
    pub fn with_cap(mut self, cap: usize) -> Self {
        self.cap = cap;
        self
    }

    /// Keep every Nth event (head-ratio sampling). `1` = keep all.
    pub fn with_sample_every(mut self, n: u64) -> Self {
        self.sample_every = n.max(1);
        self
    }

    /// Choose the FIFO key selector (per-source default, or global per-ws backstop).
    pub fn with_key(mut self, key: KeySelector) -> Self {
        self.key = key;
        self
    }

    /// True when this event passes the head-ratio sampler. Public so a test can drive the layer
    /// deterministically without fiddling with the counter.
    fn sampled(&self) -> bool {
        if self.sample_every <= 1 {
            return true;
        }
        let n = self.counter.fetch_add(1, Ordering::Relaxed);
        n % self.sample_every == 0
    }
}

impl<S> Layer<S> for SurrealCappedLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        // ONLY our own dispatch events (`target = lb.telemetry`). Without this the capped ring fills
        // with every other crate's internal `tracing` output — most damagingly SurrealDB's own
        // "Parsing SurrealQL query"/"GetR" events, which `capped_insert`'s queries emit, so the sink
        // would recursively log its own writes. The console schema only makes sense for dispatch
        // events; everything else is stderr/OTLP's job, not the bounded ring's.
        if event.metadata().target() != crate::TARGET {
            return;
        }
        if !self.sampled() {
            return;
        }
        let mut visit = FieldCollector::default();
        event.record(&mut visit);

        let record = visit.finish();
        let cap_key = match self.key {
            KeySelector::PerSource => record
                .get("source")
                .and_then(|v| v.as_str())
                .unwrap_or("_unknown")
                .to_string(),
            KeySelector::Global => record
                .get("ws")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or("_global")
                .to_string(),
        };
        let store = self.store.clone();
        let bus = self.bus.clone();
        let cap = self.cap;
        // Fire-and-forget: the ring is recent history, not a durability record. A telemetry write
        // must never block the instrumented path or kill the host when the store is slow/sick. If
        // there is no runtime to spawn onto, the event is dropped — acceptable for a sample ring.
        let spawned = tokio::spawn(write_event(store, bus, cap, cap_key, record));
        drop(spawned);
    }
}

/// Write one collected telemetry `record` into the capped ring and mirror it onto the ws-walled tail
/// subject. This is the body the Layer's `on_event` spawns; it is `pub` so a test can drive the REAL
/// write path **deterministically** (await it) instead of racing the fire-and-forget spawn. Best-effort
/// throughout: a store or bus failure is the caller's to ignore (the ring is recent history). A
/// `ws`-less record (a non-dispatch event that slipped the target filter) is stored but never published
/// — the tail is a tenant view, never cross-tenant.
pub async fn write_event(
    store: Store,
    bus: Option<Bus>,
    cap: usize,
    cap_key: String,
    record: Value,
) {
    let id = new_ulid();
    let ws = record
        .get("ws")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let payload = record.clone();
    let _ = capped_insert(&store, &ws, TABLE, &id, &cap_key, cap, &record).await;
    if let Some(bus) = bus {
        if !ws.is_empty() {
            if let Ok(bytes) = serde_json::to_vec(&payload) {
                let _ = lb_bus::publish(&bus, &ws, TAIL_SUBJECT, &bytes).await;
            }
        }
    }
}

/// Collect a `tracing` event into the stored telemetry record + its FIFO cap key, WITHOUT writing it —
/// the deterministic seam a test uses to drive [`write_event`] directly (real collection + redaction,
/// then an awaited write). Returns `None` for a non-dispatch event (wrong target) or a sampled-out one.
pub fn collect_event(layer: &SurrealCappedLayer, event: &Event<'_>) -> Option<(String, Value)> {
    if event.metadata().target() != crate::TARGET {
        return None;
    }
    if !layer.sampled() {
        return None;
    }
    let mut visit = FieldCollector::default();
    event.record(&mut visit);
    let record = visit.finish();
    let cap_key = match layer.key {
        KeySelector::PerSource => record
            .get("source")
            .and_then(|v| v.as_str())
            .unwrap_or("_unknown")
            .to_string(),
        KeySelector::Global => record
            .get("ws")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("_global")
            .to_string(),
    };
    Some((cap_key, record))
}

/// A `tracing` field visitor that collects every recorded field into a JSON object. Known fields
/// (`level`, `ws`, `actor`, `tool`, `source`, `trace_id`, `outcome`, `ts`, `msg`, `params_digest`)
/// land as top-level keys the console filters on; any other field lands under `fields`.
#[derive(Default)]
struct FieldCollector {
    level: Option<String>,
    ws: Option<String>,
    actor: Option<String>,
    tool: Option<String>,
    source: Option<String>,
    trace_id: Option<String>,
    outcome: Option<String>,
    ts: Option<u64>,
    msg: Option<String>,
    params_digest: Option<String>,
    extra: serde_json::Map<String, Value>,
}

impl FieldCollector {}

impl Visit for FieldCollector {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let s = format!("{value:?}");
        self.store_named(field.name(), Value::String(s));
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.store_named(field.name(), Value::String(value.to_string()));
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        if field.name() == "ts" {
            self.ts = Some(value);
        } else {
            self.extra
                .insert(field.name().to_string(), Value::Number(value.into()));
        }
    }
}

impl FieldCollector {
    /// Route a named string value to the known top-level field (the console filters on these) or to
    /// the `fields` bag for anything else.
    fn store_named(&mut self, name: &str, value: Value) {
        let s = match &value {
            Value::String(s) => s.clone(),
            _ => return,
        };
        match name {
            "level" | "lvl" => self.level = Some(s),
            "ws" => self.ws = Some(s),
            "actor" => self.actor = Some(s),
            "tool" => self.tool = Some(s),
            "source" => self.source = Some(s),
            "trace_id" => self.trace_id = Some(s),
            "outcome" => self.outcome = Some(s),
            "msg" | "message" => self.msg = Some(s),
            "params_digest" => self.params_digest = Some(s),
            other => {
                self.extra.insert(other.to_string(), value);
            }
        }
    }
}

impl FieldCollector {
    /// Build the stored JSON value: the known fields top-level (empty-string default for the
    /// required ones so `ws` is always present for the read-surface wall), plus `fields` for extras.
    fn finish(self) -> Value {
        let mut obj = serde_json::Map::new();
        obj.insert(
            "level".into(),
            Value::String(self.level.unwrap_or_default()),
        );
        obj.insert("ws".into(), Value::String(self.ws.unwrap_or_default()));
        obj.insert(
            "actor".into(),
            Value::String(self.actor.unwrap_or_default()),
        );
        obj.insert("tool".into(), Value::String(self.tool.unwrap_or_default()));
        obj.insert(
            "source".into(),
            Value::String(self.source.unwrap_or_default()),
        );
        obj.insert(
            "trace_id".into(),
            Value::String(self.trace_id.unwrap_or_default()),
        );
        obj.insert(
            "outcome".into(),
            Value::String(self.outcome.unwrap_or_default()),
        );
        obj.insert("ts".into(), Value::Number(self.ts.unwrap_or(0).into()));
        obj.insert("msg".into(), Value::String(self.msg.unwrap_or_default()));
        obj.insert(
            "params_digest".into(),
            Value::String(self.params_digest.unwrap_or_default()),
        );
        obj.insert("fields".into(), Value::Object(self.extra));
        Value::Object(obj)
    }
}
