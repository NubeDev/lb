//! `GET /health` — the fleet health probe (issue #72). The one unauthenticated route an
//! LB/orchestrator probes to ask "is this node up?" without a session token. The contract is
//! decided fleet-wide in `docs/scope/deploy/containerize-scope.md` §"The health contract" and
//! ratified by both fleet scopes (`rubixd`, `rartifacts`); this is the gateway's implementation of
//! the same contract every embedder (`rubix-ai`, `ems-node`, …) inherits.
//!
//! **`/health`, never `/healthz`.** One route, no `/livez`/`/readyz`/`/startupz` — the
//! liveness/readiness split is carried by the status code:
//!
//! ```text
//! GET /health  →  200  {"status":"ok",       "version":"…", "detail":{"store":"ok","gateway":"ok"}}
//!              →  503  {"status":"degraded", "version":"…", "detail":{"store":"…","gateway":"…"}}
//! ```
//!
//! - **200 = serving** — take traffic.
//! - **503 = alive but not serving** — the process answers, so a restart-on-connection-failure
//!   supervisor correctly leaves it alone while an LB that de-registers on non-200 stops sending
//!   traffic.
//! - **Connection refused = dead** — restart it. The absence of an answer is the liveness signal.
//!
//! **Reads in-memory state only** — no store query, no disk I/O, no network call. A health check
//! that can block on a dependency is a health check that can hang, and a health check that hangs is
//! a health check that lies. The [`HealthGate`] is the in-memory cell the route reads (one
//! `AtomicBool` per subsystem the contract names). Today both subsystems are always `ok`, which is
//! the *honest* answer at this layer: the store handle is alive once [`Node::boot`] opened it (the
//! gateway is constructed after, so the handle exists for every probe the route can ever serve),
//! and `gateway` is tautologically `ok` while it is handling a request. `docs/scope/system-map/
//! system-map-scope.md` already notes "the handle exists" is not real liveness, and this route
//! does not pretend otherwise — there is no store ping here. The `degraded` setters are the seam a
//! FUTURE in-process monitor (a store-down detector, a drain-shutdown handoff) flips without the
//! route shape changing; no caller flips them today.
//!
//! **Leaks nothing** beyond `status` + `version`, and `detail` names *which* subsystem is
//! degraded — never a path, DSN, or key.
//!
//! Always on when `GatewayMode::Addr`; embedders need no `BootConfig` field for it. Sits OUTSIDE
//! the auth wall (an LB has no bearer token) — the same posture as the unauthenticated
//! `POST /login`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;

use crate::state::Gateway;

/// The version this gateway reports in the health body. `env!("CARGO_PKG_VERSION")` resolves at
/// compile time to THIS crate's version — a stable identifier for "which lb-gateway build is
/// running" that an LB/orchestrator can pin a matcher on. This is the `version` field the health
/// contract documents; it leaks nothing the request path does not already imply.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const OK: &str = "ok";
const DEGRADED: &str = "degraded";

/// The in-memory health cell the gateway reads on every `/health` probe. One atomic per subsystem
/// the contract names (`store`, `gateway`); load-only reads, so a probe never blocks on a
/// dependency. Shared behind `Arc` so the route (cheap `Clone`d into each request by axum) and any
/// future in-process monitor address one source of truth.
///
/// Both subsystems default to `true` (serving) — see the module docs for why that is the honest
/// answer today rather than a store ping. [`HealthGate::set_store`] / [`HealthGate::set_gateway`]
/// are the seams a future monitor flips; no caller flips them yet.
#[derive(Debug, Default)]
pub struct HealthGate {
    store: AtomicBool,
    gateway: AtomicBool,
}

impl HealthGate {
    /// A serving gate (both subsystems `ok`) — the construction [`Gateway::build`] installs.
    pub fn new() -> Self {
        Self {
            store: AtomicBool::new(true),
            gateway: AtomicBool::new(true),
        }
    }

    /// Set the `store` subsystem state (`true` = ok, `false` = degraded) — the seam a future
    /// store-down monitor flips. No caller flips it today (see the module docs).
    pub fn set_store(&self, ok: bool) {
        self.store.store(ok, Ordering::Relaxed);
    }

    /// Set the `gateway` subsystem state (`true` = ok, `false` = degraded) — the seam a future
    /// self-degrade path (e.g. a drain-on-shutdown handoff) flips.
    pub fn set_gateway(&self, ok: bool) {
        self.gateway.store(ok, Ordering::Relaxed);
    }

    fn store_ok(&self) -> bool {
        self.store.load(Ordering::Relaxed)
    }

    fn gateway_ok(&self) -> bool {
        self.gateway.load(Ordering::Relaxed)
    }
}

/// The per-subsystem status map in the response `detail`. Values are `"ok"` or `"degraded"` only —
/// the route never puts a path, DSN, or key here.
#[derive(Debug, Serialize)]
pub struct HealthDetail {
    store: &'static str,
    gateway: &'static str,
}

/// The `/health` body — `status` + `version` + `detail`, exactly the contract shape.
#[derive(Debug, Serialize)]
pub struct HealthBody {
    status: &'static str,
    version: &'static str,
    detail: HealthDetail,
}

/// `GET /health` — unauthenticated, in-memory, one route. `200` when every subsystem is serving,
/// `503` when any is degraded (alive but not serving). Reads only the [`HealthGate`] atomics — no
/// store query, no disk I/O, no network call. See the module docs for the full contract.
pub async fn health(State(gw): State<Gateway>) -> (StatusCode, Json<HealthBody>) {
    let gate: &HealthGate = &gw.health;
    let (store_ok, gateway_ok) = (gate.store_ok(), gate.gateway_ok());
    let detail = HealthDetail {
        store: if store_ok { OK } else { DEGRADED },
        gateway: if gateway_ok { OK } else { DEGRADED },
    };
    let serving = store_ok && gateway_ok;
    let code = if serving {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    let status = if serving { OK } else { DEGRADED };
    (
        code,
        Json(HealthBody {
            status,
            version: VERSION,
            detail,
        }),
    )
}

/// A shared health gate, the shape [`Gateway`] holds. Convenience alias so [`state`] names the type
/// without reaching into the route's response privates.
pub type SharedHealthGate = Arc<HealthGate>;
