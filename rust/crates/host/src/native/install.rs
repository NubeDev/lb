//! `install_native` — the native tier's install/start verb: persist the durable records, then spawn
//! and supervise the child (native-tier scope). The peer of `install_extension` (wasm) and
//! `install_from_registry` (the signed pull in front). It composes, it does not re-invent:
//!   1. the **capability gate** (`authorize_native`) — workspace-first, `mcp:native.install:call`;
//!   2. the **S4 durable install** — persist `requested ∩ admin_approved` as the `Install` record
//!      (the same grant computation the wasm tier uses; nothing requested is live unless approved);
//!   3. the **supervisor** — build the spec (injecting the scoped identity), spawn the child, and
//!      keep the live handle in the runtime `SidecarMap` (never the store — the PID is motion);
//!   4. the **status projection** — record `native_status = {Started, restart_count: 0}` so a
//!      restart re-derives from durable state (no durable state lost, §3.4).
//!
//! Two independent gates hold: the capability gate here, and (when the binary came from the signed
//! registry) the signature gate in `pull` — installing a native extension does not bypass either.

use lb_assets::{read_install, record_install, Install};
use lb_ext_loader::{grant, Manifest};
use lb_mcp::ToolDescriptor;
use lb_supervisor::{Launcher, Sidecar};

use super::error::NativeServiceError;
use super::spec::{build_spec, native_of, tool_names};
use super::status::{record_status, NativeStatus};
use crate::boot::Node;
use crate::ui_decl::project;

/// What a native install produced — the granted caps and the child's declared tool names (for the
/// caller to surface/audit), mirroring the wasm `Loaded`.
#[derive(Debug, Clone)]
pub struct Supervised {
    pub granted_caps: Vec<String>,
    pub tools: Vec<String>,
    pub version: String,
}

/// Install (or restart-into) `manifest_toml`'s native extension in workspace `ws` for `caller`,
/// spawning the child via `launcher`. `install_dir` resolves the binary path; `admin_approved` is
/// the approved cap set; `ts` is the injected logical timestamp. Idempotent on `ext_id`: a second
/// install stops the running child first (an upgrade/re-install in place), then spawns the new one.
///
/// Authorization (`mcp:native.install:call`, workspace-first) runs FIRST — a caller without the
/// grant is refused before any record is written or any process is spawned (the deny path).
pub async fn install_native<L: Launcher>(
    node: &Node,
    launcher: &L,
    caller: &lb_auth::Principal,
    ws: &str,
    manifest_toml: &str,
    install_dir: &str,
    admin_approved: &[String],
    ts: u64,
) -> Result<Supervised, NativeServiceError> {
    super::authorize::authorize_native(caller, ws, "install")?;

    let manifest =
        Manifest::parse(manifest_toml).map_err(|e| NativeServiceError::NotNative(e.to_string()))?;
    let native = native_of(&manifest)
        .ok_or_else(|| NativeServiceError::NotNative(format!("{} is not native", manifest.id)))?;

    // A re-install RECOMPUTES `requested ∩ admin_approved` from the manifest + the binary's approved
    // set — which alone would DROP any endpoint approved at runtime (an admin registering a source
    // from the UI self-approves its `net:*` via `federation::net::grant_endpoint`, appending to this
    // record). Boot re-installs on every start, so without this carry-forward every UI-registered
    // endpoint dies at the next restart and its `test`/`query` is refused opaquely pre-connect.
    // Carry forward the prior record's runtime-added `net:*` grants so the two writers compose.
    //
    // This is NOT a widening of the wall: each carried grant was already written through an
    // admin-gated verb (the add is `mcp:datasource.add:call`), and the wall (`enforce_endpoint`)
    // still reads exactly this persisted set. Nothing here names an extension (§10) — it preserves
    // the `net:*` surface generically for any native tier extension that self-approves an endpoint.
    let granted = carry_runtime_net_grants(
        grant(&manifest, admin_approved),
        read_install(&node.store, ws, &manifest.id)
            .await
            .ok()
            .flatten(),
    );
    let tools = tool_names(&manifest);

    // STATE first: the durable approved-grant record (the same S4 verb, now for native tier). A
    // native extension may ALSO ship a page + widgets (`fleet-monitor` does), so project its UI
    // contributions onto the install exactly as the wasm tier does — narrowed to `granted` — so
    // `ext.list` surfaces the nav slot + palette tiles regardless of tier.
    let (ui, widgets) = project(&manifest, &granted);
    let install = Install::new(
        manifest.id.clone(),
        manifest.version.clone(),
        granted.clone(),
        ts,
    )
    .with_tier(lb_assets::Tier::Native)
    .with_ui(ui, widgets)
    .with_nodes(manifest.nodes.clone());
    record_install(&node.store, ws, &install).await?;

    // Make the extension's PAGE/WIDGET surface reachable by workspace admins WITHOUT any login edit
    // (authz-grants scope: "granting an extension's tool to a user/team is an ordinary grant"). We
    // grant each declared `[ui]`/`[[widget]]` scope tool — narrowed to what was actually `granted` —
    // to the `workspace-admin` role, so `resolve_caps` folds them into an admin's token on next login.
    // Generic + revocable (admin console): no per-extension code touches the login path.
    crate::authz::grant_ui_scope_to_admin(&node.store, ws, &manifest, &granted).await;

    // If a sidecar for this id is already running here, stop it before swapping (re-install in
    // place — the durable id/records stay stable, only the process is replaced).
    stop_if_running(node, ws, &manifest.id).await;

    // Spawn the child with its scoped identity, and hold the live handle in the runtime map.
    // Mint the child token with the NODE's key (so the gateway verifies it on the callback) and tell
    // the child where to POST its `/mcp/call` callbacks via `LB_GATEWAY_URL` (native-callback-transport
    // scope). The URL is deployment config the boot layer sets; unset → no callback address injected
    // and the child's callback client fails cleanly (a sidecar that never calls back is unaffected).
    let key = node.key();
    // Ask the NODE where its gateway is, not the process environment. The node knows its own address
    // (the boot layer installs it beside the signing key); reading `LB_GATEWAY_URL` instead made the
    // child's callback address depend on whether some *other* component had set that var before this
    // spawn ran — which silently broke the moment a second spawn path (boot bring-up) ran earlier
    // than the one that set it, leaving boot-respawned children with no callback address at all.
    //
    // The env var remains a fallback for an embedder that sets it and never calls
    // `install_gateway_url`; the node's own value wins when both are present.
    let gateway_url = node
        .gateway_url()
        .or_else(|| std::env::var("LB_GATEWAY_URL").ok());
    let spec = build_spec(
        native,
        install_dir,
        ws,
        &manifest.id,
        &granted,
        &key,
        gateway_url.as_deref(),
    );
    let sidecar = Sidecar::spawn(spec, launcher).await?;
    node.sidecars.insert(ws, &manifest.id, sidecar);

    // Make the native sidecar first-class in the ONE MCP routing registry (Tier-agnostic): register
    // a `SidecarDispatch` adapter under the manifest id with its declared tool descriptors, so
    // `resolve`/`dispatch`/`serve_call` reach it exactly like a wasm ext — a routed cross-node call
    // to a native sidecar now answers with ZERO Tier knowledge in the call path (§3.1). The adapter
    // holds `Arc<SidecarMap>` + id (node-global) and resolves `(ws, ext_id)` per call, so the
    // registry entry serves every workspace's child while the workspace wall stays structural. This
    // is idempotent on id like the wasm registry (a re-install swaps the entry in place).
    // The registry matches on BARE tool names (the `<ext>.` prefix is the host's routing concern —
    // `dispatch`/`serve_call` unqualify before calling the target, exactly as for a wasm ext). A
    // native manifest MAY declare its tools already-qualified (`<ext>.<tool>`, the sidecar's own ABI
    // shape); strip that prefix here so `resolve` matches the unqualified name the call path passes.
    // The adapter re-qualifies with `ext_id` before handing the name to the child (its ABI expects
    // the qualified form). A bare-name manifest is unaffected.
    let descriptors = tools
        .iter()
        .map(|t| {
            let bare = t.strip_prefix(&format!("{}.", manifest.id)).unwrap_or(t);
            ToolDescriptor::name_only(bare)
        })
        .collect();
    let adapter = super::call::SidecarDispatch::new(node.sidecars.clone(), manifest.id.clone());
    node.registry.register_local_dispatch(
        manifest.id.clone(),
        descriptors,
        std::sync::Arc::new(tokio::sync::Mutex::new(adapter)),
    );

    // Durable status: Started, restart_count 0 — what a boot reconciler (follow-up) re-derives from.
    record_status(
        &node.store,
        ws,
        &NativeStatus::new(&manifest.id, &manifest.version, ts),
    )
    .await?;

    Ok(Supervised {
        granted_caps: granted,
        tools,
        version: manifest.version,
    })
}

/// Fold the prior install's runtime-added `net:*` grants into a freshly recomputed `granted` set.
///
/// The manifest-∩-approved recompute is authoritative for every OTHER surface (a cap the admin
/// un-approves genuinely disappears on re-install — that revocation must keep working). Only `net:*`
/// is carried, because it is the one surface a runtime verb appends to AFTER install: an admin-gated
/// endpoint self-approval. Duplicates are skipped so a repeated boot cannot grow the record.
fn carry_runtime_net_grants(mut granted: Vec<String>, prior: Option<Install>) -> Vec<String> {
    let Some(prior) = prior else {
        return granted;
    };
    for g in prior.granted {
        if g.starts_with("net:") && !granted.contains(&g) {
            granted.push(g);
        }
    }
    granted
}

#[cfg(test)]
mod carry_grant_tests {
    use super::*;

    fn prior(granted: &[&str]) -> Option<Install> {
        Some(Install::new(
            "x",
            "1",
            granted.iter().map(|s| s.to_string()).collect(),
            0,
        ))
    }

    /// The regression: an endpoint self-approved from the UI must survive the boot recompute.
    #[test]
    fn carries_runtime_net_grant_absent_from_recompute() {
        let out = carry_runtime_net_grants(
            vec!["net:tls:127.0.0.1:5433:connect".into()],
            prior(&[
                "net:tls:127.0.0.1:5433:connect",
                "net:tls:tsdb.acme.com:5434:connect",
            ]),
        );
        assert!(out.contains(&"net:tls:tsdb.acme.com:5434:connect".to_string()));
    }

    /// Revocation of a NON-net cap must still take effect — the recompute stays authoritative.
    #[test]
    fn does_not_carry_non_net_grants() {
        let out = carry_runtime_net_grants(vec![], prior(&["mcp:federation.query:call"]));
        assert!(out.is_empty());
    }

    /// A repeated boot must not grow the record.
    #[test]
    fn is_idempotent_across_boots() {
        let approved = vec!["net:tls:127.0.0.1:5433:connect".to_string()];
        let once = carry_runtime_net_grants(approved.clone(), prior(&["net:tls:h:1:connect"]));
        let twice = carry_runtime_net_grants(approved, prior(&["net:tls:h:1:connect"]));
        assert_eq!(once, twice);
        assert_eq!(once.len(), 2);
    }

    /// A first install (no prior record) is unchanged.
    #[test]
    fn no_prior_install_is_a_passthrough() {
        let out = carry_runtime_net_grants(vec!["net:tls:h:1:connect".into()], None);
        assert_eq!(out, vec!["net:tls:h:1:connect".to_string()]);
    }
}

/// Stop a running sidecar for `(ws, ext_id)` if present (a cooperative shutdown). Used by a
/// re-install to replace the child in place. No-op if nothing is running here.
pub(crate) async fn stop_if_running(node: &Node, ws: &str, ext_id: &str) {
    if let Some(handle) = node.sidecars.remove(ws, ext_id) {
        handle.lock().await.shutdown().await;
    }
}
