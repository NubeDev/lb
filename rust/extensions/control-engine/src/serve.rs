//! The control-line serve loop + tool router (FILE-LAYOUT: the thin `main` lives in `main.rs`, the
//! loop + dispatch here). Reads `Content-Length`-framed `lb-supervisor` requests off stdio, answers
//! `Init`/`Health`/`Shutdown`, and routes each `Call` by family:
//!
//!   - registry verbs (`control-engine.appliance.*`) reach the `ce_appliance` table through the host
//!     `store.*` callback (`HostCtx`);
//!   - graph verbs (`control-engine.tree`/`.schema`) resolve the `appliance` selector to a CE base
//!     (`resolve`), bind a CE client (`engine::Registry`), and dispatch to the trait (`tools`).
//!
//! Stateless (§3.4): the registry is in SurrealDB (read per call); the CE client cache is a pure
//! connection pool a kill + respawn rebuilds.

use lb_supervisor::{read_frame, write_frame, CallParams, Method, Reply, Request};
use serde_json::Value;
use tokio::io::{stdin, stdout};

use crate::engine::Registry;
use crate::host::{HostCtx, HostError};
use crate::tools;
use crate::watch::WatchRegistry;

/// Run the control loop until the host closes the line (or a `Shutdown`). The binary's `main` is a
/// thin wrapper over this.
pub async fn serve() {
    let ext_id = std::env::var("LB_EXT_ID").unwrap_or_default();
    let registry = Registry::new();
    // The live-watch pump registry (S6): an in-memory connection pool, not durable state (§3.4).
    let watches = WatchRegistry::new();

    let mut input = stdin();
    let mut output = stdout();

    loop {
        let body = match read_frame(&mut input).await {
            Ok(b) => b,
            Err(_) => break, // host closed the line — exit cleanly
        };
        let req: Request = match serde_json::from_slice(&body) {
            Ok(r) => r,
            Err(_) => continue,
        };

        let reply = match req.method {
            Method::Init => Reply::ok(req.id, format!(r#"{{"ready":true,"ext":"{ext_id}"}}"#)),
            Method::Health => Reply::ok(req.id, "ok"),
            Method::Shutdown => {
                let bytes = serde_json::to_vec(&Reply::ok(req.id, "bye")).unwrap();
                let _ = write_frame(&mut output, &bytes).await;
                break;
            }
            Method::Call => handle_call(&registry, &watches, &req).await,
        };

        let bytes = serde_json::to_vec(&reply).unwrap();
        if write_frame(&mut output, &bytes).await.is_err() {
            break;
        }
    }
}

/// The logical timestamp for a registry write. The sidecar is an edge process with no clock-free core
/// contract of its own (mirrors the ROS sidecar), so wall-clock here is acceptable — it never feeds a
/// core ordering key.
fn now_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Handle a `call`: parse the tool + input, then route by family.
async fn handle_call(registry: &Registry, watches: &WatchRegistry, req: &Request) -> Reply {
    let params: CallParams = match serde_json::from_str(&req.params) {
        Ok(p) => p,
        Err(e) => return Reply::err(req.id, format!("bad params: {e}")),
    };
    let input: Value = match serde_json::from_str(&params.input) {
        Ok(v) => v,
        Err(e) => return Reply::err(req.id, format!("bad input json: {e}")),
    };

    match dispatch(registry, watches, &params.tool, &input).await {
        Ok(v) => Reply::ok(req.id, v.to_string()),
        Err(e) => Reply::err(req.id, host_err_message(e)),
    }
}

/// The tool router: registry family (`control-engine.appliance.*`) vs graph verbs. Each family builds
/// the `HostCtx` (callback + grant) it needs; the graph family additionally resolves the appliance and
/// binds a CE client. Returns the verb's JSON result or a `HostError`.
async fn dispatch(
    registry: &Registry,
    watches: &WatchRegistry,
    tool: &str,
    input: &Value,
) -> Result<Value, HostError> {
    if let Some(verb) = tool.strip_prefix("control-engine.appliance.") {
        let host = HostCtx::from_env()?;
        return match verb {
            "add" => tools::appliance::add::run(&host, input, now_ts()).await,
            "list" => tools::appliance::list::run(&host).await,
            // `remove` force-disarms any live watch for the appliance (S6) before deleting the record.
            "remove" => tools::appliance::remove::run(&host, watches, input).await,
            other => Err(HostError::BadInput(format!(
                "unknown tool: control-engine.appliance.{other}"
            ))),
        };
    }

    // The live COV feed (S6): a self-contained arm — resolve, derive the series, spawn the pump.
    if tool == "control-engine.watch" {
        let host = HostCtx::from_env()?;
        return crate::watch::verb::run(&host, registry, watches, input).await;
    }

    // A graph verb — resolve the appliance selector to a CE base, bind the client, dispatch.
    let selector = input
        .get("appliance")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let base = resolve_base(selector).await?;
    let bound = bind(registry, &base).map_err(HostError::BadResponse)?;

    // Graph WRITE verbs thread a `HostCtx` so each self-checks its own per-verb cap FIRST — the
    // inbound `native.call` carries no caller identity, so this is the finer gate the host's coarse
    // `mcp:native.call:call` cannot express (mirrors the S4 registry verbs). Reads stay ungated here
    // (S3's concern) — deny is enforced host-side by `authorize_tool` on the routed/native.call hop.
    if tools::is_write_verb(tool) {
        // Grant-only: the write path self-checks its per-verb cap but reaches CE directly (no
        // `store.*` callback), so it must not require a gateway address (the real-engine/routing
        // dev tiers run the sidecar with none).
        let host = HostCtx::grant_only_from_env();
        return tools::dispatch_write(&host, &*bound.engine, &bound.instance, tool, input).await;
    }

    // `control-engine.tree` (REAL appliance) reads via a TOLERANT raw JSON fetch, NOT
    // the crate's typed `get_tree`: `rubix-ce`'s strict `EdgeDto` (required
    // `source_uid`/`target_uid`) fails the whole decode when the engine emits a
    // dangling edge, blanking the canvas. The wiresheet consumes the `{nodes,edges}`
    // JSON verbatim, so the strict typed hop only adds a crash (see `tools::raw_tree`).
    // The fake path keeps the typed `get_tree` — it drives the trait, not real HTTP.
    if tool == "control-engine.tree" && !is_fake() {
        return tools::raw_tree::run(&base, input)
            .await
            .map_err(HostError::BadResponse);
    }

    tools::dispatch(&*bound.engine, &bound.instance, tool, input)
        .await
        .map_err(HostError::BadResponse)
}

/// True when the sanctioned in-memory stub is serving (`ce-fake` feature +
/// `LB_CE_FAKE=1`) — the host integration/routing test path. Always `false` in a
/// shipped binary, so the real read always takes the tolerant raw fetch.
fn is_fake() -> bool {
    #[cfg(feature = "ce-fake")]
    {
        return std::env::var("LB_CE_FAKE").as_deref() == Ok("1");
    }
    #[cfg(not(feature = "ce-fake"))]
    {
        false
    }
}

/// Resolve a graph verb's `appliance` selector to a CE base. Under the `ce-fake` feature + `LB_CE_FAKE=1`
/// the base is irrelevant (the fake ignores it), so skip the registry lookup entirely — the host
/// integration/routing tests drive the fake without seeding a `ce_appliance` record. Otherwise resolve
/// against the `ce_appliance` registry (workspace-walled; unknown/other-ws → not-found).
async fn resolve_base(selector: &str) -> Result<String, HostError> {
    #[cfg(feature = "ce-fake")]
    {
        if std::env::var("LB_CE_FAKE").as_deref() == Ok("1") {
            return Ok(selector.to_string());
        }
    }
    // If the host callback cannot even be built (no `LB_GATEWAY_URL`/token — the real-engine dev tier
    // that runs the sidecar without a gateway), there is no registry to consult: fall back to the
    // literal selector as a base, exactly as `resolve` does when the store is unreachable. With a real
    // gateway present this path is never taken, so it cannot leak isolation.
    let host = match HostCtx::from_env() {
        Ok(h) => h,
        Err(_) => return Ok(selector.to_string()),
    };
    let resolved = crate::resolve::resolve(&host, selector).await?;
    Ok(resolved.base)
}

/// Resolve a CE base to a bound CE client. Under the `ce-fake` feature AND `LB_CE_FAKE=1`, serve the
/// sanctioned in-memory stub instead — the host integration/routing test path, so it can drive the REAL
/// supervisor + gate + stdio ABI without the C++ engine. OFF in a shipped binary.
fn bind(registry: &Registry, base: &str) -> Result<crate::engine::Bound, String> {
    #[cfg(feature = "ce-fake")]
    {
        if std::env::var("LB_CE_FAKE").as_deref() == Ok("1") {
            return Ok(crate::engine::Bound {
                engine: crate::ce_fake::CeFake::seeded(),
                instance: rubix_ce::EngineInstanceId::edge(),
            });
        }
    }
    registry.get(base)
}

/// Flatten a `HostError` to the wire error string. `Denied`/`NotFound` are the opaque, well-known
/// tokens the host maps back onto `ToolError`; the rest carry a diagnostic (never the token).
fn host_err_message(e: HostError) -> String {
    match e {
        HostError::Denied => "denied".into(),
        HostError::NotFound => "not found".into(),
        other => other.to_string(),
    }
}
