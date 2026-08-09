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
//!
//! `version` is **this gateway crate's** build and always has been. A node booted by an embedder
//! additionally reports `"product":{"name":"…","version":"…"}` — the program on top — which is
//! omitted entirely otherwise; see `routes::product` for why that is additive and never a rename.
//! ```
//!
//! - **200 = serving** — take traffic.
//! - **503 = alive but not serving** — the process answers, so a restart-on-connection-failure
//!   supervisor correctly leaves it alone while an LB that de-registers on non-200 stops sending
//!   traffic.
//! - **Connection refused = dead** — restart it. The absence of an answer is the liveness signal.
//!
//! ## One conditional field: `trust_gate`
//!
//! A node running the development escape hatch `LB_EXT_UNTRUSTED_KEY=allow` (see
//! `session::trusted`) additionally reports:
//!
//! ```text
//! GET /health  →  200  {"status":"ok", …, "trust_gate":"waived-untrusted-key"}
//! ```
//!
//! The field is **omitted entirely** on a normally-configured node, so the contract above is
//! unchanged for every existing probe — nothing new to parse unless there is something wrong to
//! report. The status code is unaffected: a waived gate is a deliberate configuration, not a
//! degraded subsystem, and 503 here would evict a healthy bench node from an LB.
//!
//! Why an unauthenticated route carries it at all: the failure mode this feature exists to prevent
//! is a bench setting silently surviving into production, and the person who discovers that is
//! usually someone who inherited the box and has no credentials for it yet. Surfacing the posture
//! where they will actually look beats hiding it behind auth they do not have. The trade is real —
//! anyone who can reach the port learns this node accepts foreign-signed extensions — so the value
//! is a bare posture marker and never names a key, publisher, or path, and the knob must not be
//! enabled on a node facing an untrusted network. It is also still reported on the two log surfaces
//! (boot, and every waived artifact) for operators who prefer to keep the wire quiet.
//!
//! ## A second conditional field: `store_bounds`
//!
//! A node whose store has no bound on how much it can accumulate additionally reports:
//!
//! ```text
//! GET /health  →  200  {"status":"ok", …, "store_bounds":"unbounded-bytes"}
//! ```
//!
//! Same shape and same trade as `trust_gate` above, for the same reason: **omitted entirely** when
//! every growth term is bounded (and when nobody has reported, so an embedder that never calls
//! [`HealthGate::set_store_bounds`] sees a byte-identical body), and the status code is unaffected.
//! An unbounded store is a deliberate — or, far more often, an accidentally-reverted —
//! configuration, not a degraded subsystem. It is serving perfectly well right now; that is exactly
//! why it is dangerous, and why 503 would be the wrong answer (it would evict a node that is fine
//! *today* and tell an operator nothing about *why*).
//!
//! Why it exists: rubix-ai#84. A node was rebuilt, came back with its rollup retention bound and its
//! byte budget both silently gone, grew ~1.09 GB of commit log in ~2.5 h, and then boot-looped
//! against the store memory guard. Nothing said so until the disc was nearly full. The bounds are
//! logged at boot by whoever fills this cell, but a log line on one box is not something a fleet
//! monitor can poll — this route is, and it is the one an orchestrator already scrapes.
//!
//! The value is a **bare posture marker naming which TERM is unbounded** — `bytes`, `rows`, or
//! both. Never a byte count, a series prefix, a policy, or a path. That keeps the unauthenticated
//! trade the same as `trust_gate`'s: someone who can reach the port learns this node is not bounded,
//! which is a thing they can act on, and learns nothing about what it stores.
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
//! `POST /auth/login`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;

use crate::routes::ProductBody;
use crate::state::Gateway;

/// The version this gateway reports in the health body. `env!("CARGO_PKG_VERSION")` resolves at
/// compile time to THIS crate's version — a stable identifier for "which lb-gateway build is
/// running" that an LB/orchestrator can pin a matcher on. This is the `version` field the health
/// contract documents; it leaks nothing the request path does not already imply.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const OK: &str = "ok";
const DEGRADED: &str = "degraded";

/// The `trust_gate` value reported when the publisher-signature check has been waived
/// (`LB_EXT_UNTRUSTED_KEY=allow`). Names the posture, never the allow-list contents.
const TRUST_GATE_WAIVED: &str = "waived-untrusted-key";

/// The `store_bounds` markers. Bare postures naming the unbounded TERM and nothing else — no byte
/// count, no series prefix, no policy, no path (see the module docs on the unauthenticated trade).
const UNBOUNDED_BYTES: &str = "unbounded-bytes";
const UNBOUNDED_ROWS: &str = "unbounded-rows";
const UNBOUNDED_BOTH: &str = "unbounded-bytes+rows";

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
    /// Has anyone reported the store's growth bounds? `false` (the default) ⇒ the `store_bounds`
    /// field is omitted entirely, which is what every node that does not fill this cell reports —
    /// including the stock binary, so its body is unchanged.
    ///
    /// Tracked separately from the two flags below rather than folded into them, because "nobody
    /// looked" and "everything is bounded" are different claims and only one of them is a promise.
    /// Defaulting an unreported node to `bounded` would be the exact lie this feature exists to
    /// stop telling.
    bounds_reported: AtomicBool,
    /// Is the store bounded in BYTES (a disk budget is in force)?
    bytes_bounded: AtomicBool,
    /// Is the store bounded in ROWS (every retention term has a horizon or a cap)?
    ///
    /// Separate from `bytes_bounded` because they are different nouns and bounding one does not
    /// bound the other: rubix-ai's `disc-failsafe-scope.md` §0 is a live case where every row bound
    /// held and the node still came within hours of a full disc, because an append-only commit log
    /// is reclaimed by compaction and nothing else.
    rows_bounded: AtomicBool,
}

impl HealthGate {
    /// A serving gate (both subsystems `ok`) — the construction [`Gateway::build`] installs.
    pub fn new() -> Self {
        Self {
            store: AtomicBool::new(true),
            gateway: AtomicBool::new(true),
            // UNREPORTED, not "bounded". Nobody has looked yet, and the two are different claims —
            // see the field docs. Every node that never calls `set_store_bounds` (including the
            // stock binary) therefore omits the field and reports a byte-identical body.
            bounds_reported: AtomicBool::new(false),
            bytes_bounded: AtomicBool::new(false),
            rows_bounded: AtomicBool::new(false),
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

    /// Report whether this node's store is bounded in each of its two growth terms.
    ///
    /// The seam an embedder calls once, after boot, with whatever it worked out about its own
    /// configuration — this crate does not know what a budget or a retention policy is, and must
    /// not (rule 10). Idempotent and callable again if a node ever learns better.
    ///
    /// Until it is called the field is omitted, so calling nothing changes nothing.
    pub fn set_store_bounds(&self, bytes_bounded: bool, rows_bounded: bool) {
        self.bytes_bounded.store(bytes_bounded, Ordering::Relaxed);
        self.rows_bounded.store(rows_bounded, Ordering::Relaxed);
        // LAST, and deliberately: a probe racing this call must never see "reported" alongside the
        // default flags. Relaxed ordering is enough because these are three independent facts read
        // one at a time, not a lock — the worst a racing probe sees is the previous report.
        self.bounds_reported.store(true, Ordering::Relaxed);
    }

    /// The `store_bounds` marker, or `None` when unreported or fully bounded (⇒ field omitted).
    fn store_bounds(&self) -> Option<&'static str> {
        if !self.bounds_reported.load(Ordering::Relaxed) {
            return None;
        }
        match (
            self.bytes_bounded.load(Ordering::Relaxed),
            self.rows_bounded.load(Ordering::Relaxed),
        ) {
            (true, true) => None,
            (false, true) => Some(UNBOUNDED_BYTES),
            (true, false) => Some(UNBOUNDED_ROWS),
            (false, false) => Some(UNBOUNDED_BOTH),
        }
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

/// The `/health` body — `status` + `version` + `detail`, plus `trust_gate` **only when the publisher
/// trust gate has been disabled**.
///
/// `trust_gate` is `skip_serializing_if = "Option::is_none"` on purpose: on a normally-configured
/// node the body is byte-identical to what it has always been, so no existing probe, matcher, or
/// dashboard sees a change. The field materialises exactly when there is something to say. This is
/// an operator-visibility feature (an inherited box should reveal a forgotten bench setting without
/// anyone reading the unit file); the cost is that it is also visible unauthenticated, which is why
/// the value is a bare marker naming no key, path, or publisher.
#[derive(Debug, Serialize)]
pub struct HealthBody {
    status: &'static str,
    version: &'static str,
    detail: HealthDetail,
    #[serde(skip_serializing_if = "Option::is_none")]
    trust_gate: Option<&'static str>,
    /// Present ONLY when the store's growth has been reported AND some term is unbounded. Omitted
    /// on a bounded node and on one that never reported — see the module docs.
    #[serde(skip_serializing_if = "Option::is_none")]
    store_bounds: Option<&'static str>,
    /// What program embedded this node (`BootConfig::build_info`), or **omitted entirely** when
    /// none did — the stock binary's body is unchanged (embedder-build-info scope).
    ///
    /// `version` above keeps meaning *this gateway crate's* build, unchanged and forever; this is
    /// the version of the product on top. `/health` carries it as well as `/node` because a fleet
    /// prober usually holds only this route, and asking it to make a second call for a version it
    /// is already half-reading is a poor trade for two short strings. Same source value as
    /// `/node`'s, so the two cannot disagree. See `routes::product`.
    #[serde(skip_serializing_if = "Option::is_none")]
    product: Option<ProductBody>,
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
            // Present ONLY on a node whose publisher trust gate is disabled. Note this does not make
            // the node `degraded`: a waived gate is a deliberate configuration, not a fault, and
            // reporting 503 would pull a working bench box out of an orchestrator's rotation for a
            // condition it is supposed to be in. Loudness comes from the field, not the status code.
            trust_gate: gw.authenticity.is_waived().then_some(TRUST_GATE_WAIVED),
            // Same posture as `trust_gate`: loudness comes from the FIELD, not the status code. An
            // unbounded node is serving correctly at this instant — that is the whole problem with
            // it — so 503 would pull a working box out of rotation and still not say why.
            store_bounds: gate.store_bounds(),
            // The same boot-time cell `GET /node` reads — one value, both surfaces.
            product: ProductBody::from_build_info(gw.build_info.as_ref().as_ref()),
        }),
    )
}

/// A shared health gate, the shape [`Gateway`] holds. Convenience alias so [`state`] names the type
/// without reaching into the route's response privates.
pub type SharedHealthGate = Arc<HealthGate>;
