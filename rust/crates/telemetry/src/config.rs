//! The sink selector the `node` binary wires from **config** (observability scope, symmetric nodes
//! rule #1). `LB_TELEMETRY_SINK` chooses which `tracing-subscriber` layers install: `stderr`
//! (default), `surreal` (the capped ring), `both`, or `off`. No code branch on role — a workstation
//! an appliance and a hub all run the same binary; only the env differs.
//!
//! This is the entry layer the emission scope names: it sets the default subscriber and stacks the
//! selected layers. The `SurrealCappedLayer` is a peer; OTLP export (a future layer) slots in here
//! too without touching the emit path.

use lb_bus::Bus;
use lb_store::Store;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

use crate::layer::SurrealCappedLayer;

/// The selected telemetry sink set, parsed from `LB_TELEMETRY_SINK`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SinkConfig {
    /// A human stderr layer (the default; `RUST_LOG` filters it).
    Stderr,
    /// The capped SurrealDB ring (the in-product recent-history sink).
    Surreal,
    /// Both: stderr for the operator + the capped ring for the console.
    Both,
    /// No sink (telemetry fully off).
    Off,
}

impl SinkConfig {
    /// Parse from the `LB_TELEMETRY_SINK` env var (default `stderr`). Unknown values fall back to
    /// `stderr` (never a silent "off").
    pub fn from_env() -> Self {
        match std::env::var("LB_TELEMETRY_SINK")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "surreal" => SinkConfig::Surreal,
            "both" => SinkConfig::Both,
            "off" => SinkConfig::Off,
            _ => SinkConfig::Stderr,
        }
    }
}

/// Install the configured sink set as the global default subscriber. Call once at boot from the
/// `node` binary, BEFORE any instrumented code runs. A no-op for [`SinkConfig::Off`]. `store` is the
/// node's existing handle (the one the host opened) — the ring lives in the SAME store, not a second
/// one (rule #2). `bus` is the node's Zenoh peer so the capped ring mirrors onto the ws-walled tail
/// subject for `telemetry.tail`.
pub fn sink_layers(store: Store, bus: Bus, cfg: SinkConfig) {
    if cfg == SinkConfig::Off {
        return;
    }
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let capped = || SurrealCappedLayer::new(store.clone()).with_bus(bus.clone());
    match cfg {
        SinkConfig::Stderr => {
            tracing_subscriber::registry()
                .with(filter)
                .with(tracing_subscriber::fmt::layer())
                .init();
        }
        SinkConfig::Surreal => {
            tracing_subscriber::registry()
                .with(filter)
                .with(capped())
                .init();
        }
        SinkConfig::Both => {
            tracing_subscriber::registry()
                .with(filter)
                .with(tracing_subscriber::fmt::layer())
                .with(capped())
                .init();
        }
        SinkConfig::Off => {}
    }
}
