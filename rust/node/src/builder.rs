//! [`boot_full`] + [`RunningNode`] — the supported embed API's boot seam (node-roles / embed scope).
//!
//! [`boot_full`] performs the whole boot ritual ONCE, from a [`BootConfig`](crate::BootConfig): boot
//! the spine over the config's store/key, apply telemetry, run the `hello` demo (gated), re-load
//! enabled extensions, seed, spawn reactors (gated), then the gateway/roles block. The `node` binary
//! and any third-party embedder both call it — the binary's `main.rs` is `boot_full(from_env()).await`
//! plus `serve()`.
//!
//! **Load-bearing ordering (preserved EXACTLY from the old `main.rs`):** the native sidecar roles
//! (`federation`, `control-engine`) and the in-house `agent` are mounted AFTER the gateway installs its
//! signing key onto the node (`Gateway::new_live` → `node.install_key`). `install_native` mints each
//! sidecar's `LB_EXT_TOKEN` with `node.key()`, and the gateway verifies those callback tokens with its
//! own key — mounting before the gateway installed that key meant every callback 401'd. With the
//! gateway OFF, the node's boot key stays and the roles mount headless (their callbacks degrade
//! cleanly).
//!
//! **Boot bring-up is per-tier, and the two halves sit in different places for that same reason:**
//! wasm (`load_enabled`) loads components into the runtime and is key-independent, so it runs early;
//! native (`spawn_enabled`) spawns children whose tokens are minted with the node's key, so it runs
//! down in the roles block, AFTER the gateway installed that key. Both consume the one `reconcile`
//! plan. Calling only the wasm half is what left every published native extension dead after a
//! restart, silently (issue #64).
//!
//! Both halves run for **every active workspace** (`boot_workspaces`), not just `cfg.workspace` — a
//! node can serve many, and bringing up only the configured one stranded the rest's extensions in
//! exactly the same silent way.
//!
//! **The gateway ordering is TWO rules, not one, and the second is easy to miss.** A native child
//! needs the gateway's signing *key* installed before it spawns (so its `LB_EXT_TOKEN` verifies) —
//! that is the rule the role mounts document. But a child that loads its config through a host
//! callback also needs the gateway's socket to be **listening**, and constructing a `Gateway` does
//! not listen: the bind used to live in `RunningNode::serve()`, which the embedder calls *after*
//! `boot_full` returns. So "after the gateway block" satisfied the key rule and silently violated the
//! listen rule — every boot-spawned child POSTed into a closed port, came up with an empty runtime,
//! and reported healthy while doing nothing. The socket is therefore bound **here**, in the gateway
//! block, and handed to `serve()` already-bound; the node is told its own URL at the same moment.
//! Both rules are now satisfied by construction rather than by the order two functions happen to run.

use std::sync::Arc;

use lb_host::{load_enabled, AgentServer, Node};
use lb_role_gateway::Gateway;

use crate::config::{BootConfig, CredentialMode, GatewayMode};
use crate::open_store::open_store;

/// A booted node, ready to serve. Hands back the [`Node`] (store / bus / host verbs — the embedder
/// calls verbs in-process), and — when the gateway is on — the [`Gateway`] value + bind address to
/// serve, plus the live [`AgentServer`] kept alive for the node's lifetime (dropping it stops serving).
///
/// With the gateway OFF, `gateway` is `None` and [`serve`](RunningNode::serve) returns immediately; the
/// embedder drives the node purely in-process. Teardown beyond process exit (reactor cancellation,
/// sidecar shutdown token) is a documented follow-up — `RunningNode` deliberately owns the pieces so a
/// `shutdown()` can be added additively without changing this shape.
pub struct RunningNode {
    /// The booted node: store, bus, registry, host verbs. Call `lb_host::*` verbs in-process against it.
    pub node: Arc<Node>,
    /// The gateway value + its **already-bound listener**, when [`GatewayMode::Addr`] was configured.
    /// `None` = headless.
    ///
    /// The socket is bound inside [`boot_full`], not in [`serve`](RunningNode::serve), and that is
    /// load-bearing: a native sidecar spawned during boot loads its config through a host callback to
    /// this very gateway, so the port must be **listening before any child spawns**. Binding here and
    /// handing the listener over makes that ordering structural — `serve` cannot start "later than"
    /// a bind that already happened. Returning `addr` alone (and binding in `serve`) meant every
    /// boot-spawned child POSTed into a closed port: it came up with an empty runtime, reported
    /// `health=ok`, and did nothing, forever.
    pub gateway: Option<(Gateway, tokio::net::TcpListener)>,
    /// The served in-house agent, kept alive here (dropping it stops serving routed `agent.invoke`).
    pub agent_server: Option<AgentServer>,
    /// The live LAN-discovery advertisement, when [`BootConfig::discovery`] was set. Held here for
    /// the same reason `agent_server` is: dropping it retracts the mDNS record, so the node stops
    /// being discoverable exactly when it stops running. `None` = advertising nothing (the default).
    pub advertised: Option<lb_discovery::Advertised>,
}

impl RunningNode {
    /// Mint a **short-lived session token** for `principal_sub` in `workspace`, carrying exactly that
    /// principal's live capabilities.
    ///
    /// This is the seam for driving the node's own gateway from a non-interactive worker — a headless
    /// browser rendering a page, a job that has to call a route rather than a host fn. Such a worker
    /// needs a real bearer token, and the alternatives were all wrong: a password login is
    /// interactive and yields a 12-hour token, an API key carries no `reach:` caps (so the UI 403s at
    /// page entry) and dies on restart, and hand-rolling `Claims` re-implements the cap fold.
    ///
    /// **It grants nothing.** The caps come from `principal_sub`'s durable grants at mint time, so the
    /// worker sees precisely what that principal sees and a revoked grant takes effect on the next
    /// mint. `ttl` should be minutes, not hours — the token is minted for one job and thrown away.
    ///
    /// Returns `None` when the node was booted headless ([`GatewayMode::Off`]): with no gateway there
    /// is nothing for a token to authenticate against.
    pub async fn mint_service_session(
        &self,
        principal_sub: &str,
        workspace: &str,
        now: u64,
        ttl: std::time::Duration,
    ) -> Option<lb_role_gateway::MintedSession> {
        let (gw, _) = self.gateway.as_ref()?;
        Some(
            lb_role_gateway::mint_full_session_with_ttl(
                &self.node,
                &gw.key,
                principal_sub,
                workspace,
                now,
                ttl.as_secs(),
            )
            .await,
        )
    }

    /// Serve the gateway, blocking until it stops (never, in normal operation). A no-op that returns
    /// `Ok(())` immediately when the gateway is off — the embedder is driving the node in-process. The
    /// `agent_server` is held for the duration so routed invocations keep serving.
    pub async fn serve(self) -> anyhow::Result<()> {
        let RunningNode {
            gateway,
            agent_server,
            ..
        } = self;
        if let Some((gw, listener)) = gateway {
            // Already bound (in `boot_full`, before any sidecar spawned) — just start accepting.
            let addr = listener
                .local_addr()
                .map(|a| a.to_string())
                .unwrap_or_else(|_| "?".into());
            // Scheme-aware, for a node behind a TLS reverse proxy (rubix-fleet
            // docs/rasp-pi/HTTPS.md §4, work item 7). This line is what an
            // operator reads out of the journal and types, so once a node binds
            // loopback behind Caddy, `http://127.0.0.1:8099` is a broken
            // instruction — the front door is `https://ai.pi-07.local`.
            //
            // NOTE the deliberate asymmetry with `install_gateway_url` below:
            // that one stays plaintext loopback forever (§3). What we PRINT and
            // what sidecars CALL are different questions, and conflating them is
            // how a sidecar ends up unable to verify a cert, reporting
            // `health=ok`, and doing nothing.
            println!("gateway: serving on {}", printed_gateway_url(&addr));
            lb_role_gateway::serve_listener(gw, listener).await?;
        }
        // Hold the agent server alive until serve returns (or forever, headless would drop here).
        drop(agent_server);
        Ok(())
    }
}

/// The operator-facing gateway URL to PRINT — as opposed to the address the
/// listener bound or the loopback URL sidecars call back on.
///
/// `LB_PUBLIC_URL` wins outright (`https://ai.pi-07.local`): a node behind a
/// reverse proxy is reached at a hostname and scheme that its bind address
/// simply does not contain. Otherwise `LB_SCHEME=https` upgrades the scheme on a
/// non-loopback bind.
///
/// **A loopback address is never printed as `https://`.** The loopback listener
/// is plaintext by design (rubix-fleet docs/rasp-pi/HTTPS.md §3) and no
/// certificate carries a `127.0.0.1` SAN, so such a URL can neither connect nor
/// verify — it is a broken instruction handed to someone reading a journal to
/// find out why a box is unreachable.
fn printed_gateway_url(addr: &str) -> String {
    printed_gateway_url_from(
        addr,
        std::env::var("LB_PUBLIC_URL").ok().as_deref(),
        std::env::var("LB_SCHEME").ok().as_deref(),
    )
}

/// The pure core of [`printed_gateway_url`], split out so the rules are testable
/// without mutating process env (which no parallel test suite can do safely).
fn printed_gateway_url_from(addr: &str, public: Option<&str>, scheme: Option<&str>) -> String {
    if let Some(public) = public {
        let public = public.trim().trim_end_matches('/');
        if !public.is_empty() {
            return public.to_string();
        }
    }
    let loopback =
        addr.starts_with("127.") || addr.starts_with("localhost:") || addr.starts_with("[::1]");
    let https = matches!(scheme, Some(s) if s.trim().eq_ignore_ascii_case("https"));
    if https && !loopback {
        format!("https://{addr}")
    } else {
        format!("http://{addr}")
    }
}

/// Perform the full boot ritual from `cfg` and return a [`RunningNode`]. This is the ONE copy of the
/// ritual; the binary and every embedder call it. No env is read here below the seam — everything
/// comes from `cfg` (the exceptions are the role mounts `federation`/`control_engine`, which still read
/// their own `LB_FEDERATION_*` / `LB_CONTROL_ENGINE_*` env; that de-env'ing is an explicit documented
/// follow-up — the core ritual store/key/workspace/seeds/reactors/gateway is fully struct-config).
pub async fn boot_full(cfg: BootConfig) -> anyhow::Result<RunningNode> {
    // Boot the spine over the config's store. `Arc` so the reactors + role mounts share it.
    let node = Arc::new(Node::boot_with_store(open_store(&cfg).await?).await?);

    // RESPONSE CACHE (response-cache scope): install the optional in-process cache from config, right
    // after boot so every subsequent host-verb dispatch sees it (a headless node caches too — the win
    // is not gateway-specific). A no-op when the `page-cache` feature is off OR `cfg.cache` is
    // `None`/disabled — the zero-cost seam. Symmetric: on/off/budget is config, never a role branch.
    node.install_response_cache(cfg.cache.clone());
    // The disk budget (issue #122) — `LB_STORE_MAX_BYTES` reaches `store.status` and the
    // store-compact reactor from here. Unset ⇒ `None` ⇒ the node behaves exactly as before.
    node.install_store_budget(cfg.store_budget_bytes);

    // NODE IDENTITY: install the embedder's durable node id as this node's bus identity, replacing
    // the fresh-per-process random one `Node::boot*` mints. Installed HERE — before the gateway is
    // built and before anything declares a queryable or announces presence — so every downstream
    // consumer sees one id for the whole life of the process. Doing it later would let a routed
    // call or a roster entry be addressed under the throwaway id.
    //
    // `None` leaves the random id untouched (unchanged behaviour, correct for a test). See
    // `BootConfig::identity` for why a real deployment must set it: a per-boot id makes every
    // restart look like a brand-new node and the fleet roster accumulates ghosts.
    if let Some(identity) = &cfg.identity {
        node.install_node_id(identity.node().clone());
    }

    // TELEMETRY sink selection: choose the tracing layers by config, right after boot so every
    // subsequent instrumented call is captured. Shares the node's OWN store + bus handles.
    lb_telemetry::sink_layers(node.store.clone(), node.bus.clone(), cfg.telemetry);

    // S1 hello demo bring-up (gated): load the `hello` extension and call `hello.echo` once. The binary
    // runs it; an embedder wants it off.
    if cfg.hello_demo {
        if let Err(e) = crate::hello_demo::run(&node).await {
            eprintln!("boot: hello demo failed: {e}");
        }
    }

    // BOOT BRING-UP: re-load every previously-published-and-enabled wasm extension from the durable
    // cache, so an upload survives a restart. A no-op on a fresh store.
    //
    // For EVERY active workspace, not just `cfg.workspace`: this node may serve many (workspace.create
    // is a verb, the UI has a switcher), and bringing up only the configured one left every other
    // workspace's extensions dead after a restart — silently, the same shape as the native gap. See
    // `boot_workspaces` for why the set is a UNION with `cfg.workspace` rather than the directory
    // alone (a node whose boot workspace was never `workspace.create`d has no row for it).
    let ws = cfg.workspace.clone();
    let boot_wss = lb_host::boot_workspaces(&node.store, &ws)
        .await
        .unwrap_or_else(|_| vec![ws.clone()]);
    for w in &boot_wss {
        match load_enabled(&node, w).await {
            Ok(loaded_exts) => {
                for e in loaded_exts.iter().filter(|e| e.loaded) {
                    println!(
                        "boot-loaded extension: {}@{} ({}) ws={w}",
                        e.ext, e.version, e.reason
                    );
                }
            }
            Err(e) => eprintln!("boot extension load for ws={w} failed: {e}"),
        }
    }

    // SEEDS: dev identity, core skills, agent definitions, personas, active-persona migration, default
    // core-skill grants. All idempotent + best-effort.
    crate::seeds::run(&node, &cfg).await;

    // REACTORS (gated): flow / agent / approval / insight-digest scans + the one-shot insight-ts heal.
    if cfg.reactors {
        crate::reactors::spawn(
            &node,
            &ws,
            &cfg.outbox_providers,
            cfg.email_transport.as_ref(),
            cfg.store_budget_bytes,
            cfg.profile,
        )
        .await;
    }

    // GATEWAY + ROLES block. Mount the native roles + agent AFTER the gateway installs its signing key
    // (the load-bearing ordering — see the module doc). With the gateway off, the node's boot key stays
    // and the roles mount headless.
    let gateway = match &cfg.gateway {
        GatewayMode::Addr(addr) => {
            // A LIVE clock: `Gateway::new_live` reads wall time per request and installs its key onto
            // the node. Do NOT call `Gateway::boot()` here — that would open a second store handle.
            let mut gw = Gateway::new_live(node.clone(), cfg.signing_key.clone());
            // Select the credential check `POST /login` runs (embedder-credential-mode scope).
            // `new_live` hardwired `DevTrustAny` (password-less); apply the config's choice through
            // the existing `with_credential_check` builder so an embedded node can enforce real
            // passwords (`PasswordHash` → argon2, wrong/absent secret 401s). This is the ONLY place
            // an embedded node's login check is selected; the mode came down from `BootConfig`
            // (from_env at the binary, or the embedder's explicit choice), never re-read from env.
            let check: Arc<dyn lb_role_gateway::CredentialCheck> = match cfg.credential_mode {
                CredentialMode::DevTrustAny => Arc::new(lb_role_gateway::DevTrustAny),
                CredentialMode::PasswordHash => Arc::new(lb_role_gateway::PasswordHash),
            };
            gw = gw.with_credential_check(check);
            // Select the GLOBAL credential check `POST /auth/login` runs (email-login scope), from the
            // SAME `credential_mode` so both human doors agree: `DevTrustAny` ⇒ password-less dev/CI,
            // `PasswordHash` ⇒ real argon2 against the global credential (wrong/absent secret 401s).
            // `new_live` hardwired `GlobalDevTrustAny`; without this the embedded `/auth/login` would
            // stay password-less even under `PasswordHash`.
            let global_check: Arc<dyn lb_role_gateway::GlobalCredentialCheck> =
                match cfg.credential_mode {
                    CredentialMode::DevTrustAny => Arc::new(lb_role_gateway::GlobalDevTrustAny),
                    CredentialMode::PasswordHash => Arc::new(lb_role_gateway::GlobalPasswordHash),
                };
            gw = gw.with_global_credential_check(global_check);
            // Pin the `POST /extensions` upload ceiling from config (extension-upload-limit fix). The
            // route-scoped body limit is sized from this; the binary fills it via `from_env`
            // (`LB_MAX_EXTENSION_UPLOAD_BYTES`, default 384 MiB), an embedder sets it directly.
            gw = gw.with_max_extension_upload_bytes(cfg.max_extension_upload_bytes);
            // Pin the publisher trust-gate posture from config. `new_live` reads `LB_EXT_UNTRUSTED_KEY`
            // itself, but an EMBEDDED node must not inherit the host process's env — the mode came
            // down from `BootConfig` (from_env at the binary, or the embedder's explicit choice), the
            // same discipline `credential_mode` follows above. Default is `Required` (gate enforced).
            gw = gw.with_authenticity(cfg.authenticity);
            // Relocate the extension-UI serve dir when the embedder set one (`Some` ⇒ pin it via the
            // builder); `None` leaves the gateway's own `LB_EXT_UI_DIR`/"extensions-ui" default in place,
            // so the standalone binary is untouched (ext-UI-dir embed seam).
            if let Some(dir) = cfg.ext_ui_dir.as_deref().filter(|d| !d.is_empty()) {
                gw = gw.with_ext_ui_dir(dir);
            }
            // Serve a static web app at `/` when the embedder set a static root (static-root scope);
            // `None`/empty leaves the router with no fallback (unmatched paths 404, unchanged).
            if let Some(dir) = cfg.static_root.as_deref().filter(|d| !d.is_empty()) {
                gw = gw.with_static_root(dir);
            }
            // Terminate a cookie-backed browser session at `/api/*` when the embedder opted in
            // (browser-session scope). `None` leaves the router bearer-only, byte-for-byte as today —
            // no `/api/*` route, no cookies (rubixd, rubix-ai, every existing node).
            if let Some(bs) = cfg.browser_session.clone() {
                gw = gw.with_browser_session(bs);
            }
            // BIND NOW — before any native sidecar spawns below.
            //
            // A boot-spawned child loads its config through a host callback to THIS gateway. Binding
            // in `serve()` (which the embedder calls after `boot_full` returns) meant the child POSTed
            // into a closed port, came up with an empty runtime, and reported `health=ok` while doing
            // nothing — forever, since the child loads its config exactly once. Binding here makes the
            // ordering structural rather than a race: the socket is accepting connections before a
            // child exists to call it. `serve()` then just runs the accept loop on this listener.
            let listener = tokio::net::TcpListener::bind(addr).await?;
            // Tell the node its OWN callback address (the real one — `bind` may have resolved a
            // port-0 request to a concrete port). `install_native` reads it from here rather than
            // from a process-global `LB_GATEWAY_URL` that nothing guarantees is set by spawn time.
            let bound = listener.local_addr().unwrap_or(*addr);
            // PLAINTEXT, ALWAYS — do not make this scheme-aware.
            //
            // This is the URL boot-spawned sidecars POST their config to, and it
            // is in the plaintext-loopback set (rubix-fleet
            // docs/rasp-pi/HTTPS.md §3) for a specific reason: a sidecar that
            // cannot verify a certificate comes up with an empty runtime and
            // reports `health=ok` forever, which is the exact failure the
            // bind-here-not-in-serve() comment above exists to prevent. The
            // PRINTED url (see `printed_gateway_url`) is a different question.
            node.install_gateway_url(format!("http://{bound}"));
            // The unauthenticated `GET /node` probe (node-identity scope). Installed with the REAL
            // bound port — `bind` may have resolved a `:0` request to a concrete port, and the whole
            // value of the route is publishing something a caller can actually dial.
            //
            // A wildcard bind (`0.0.0.0`/`[::]`) is reported with NO addresses rather than the
            // wildcard itself: `0.0.0.0` is not an address a client can connect to, and handing one
            // over would be worse than silence. A caller that reached this route already has a
            // working address — its own connection's. Enumerating real interfaces is deliberately
            // not done here: it is a platform-specific concern, and no core crate should grow one
            // (rule 10).
            if let Some(identity) = cfg.identity.clone() {
                let addresses = if bound.ip().is_unspecified() {
                    Vec::new()
                } else {
                    vec![bound.ip()]
                };
                gw = gw.with_identity(identity, bound.port(), addresses);
            }
            Some((gw, listener))
        }
        GatewayMode::Off => None,
    };

    // Now there is ONE signing identity (the gateway installed it, or the node's boot key stands).
    // Mount the native sidecar roles + the in-house agent HERE so a served run's tool callbacks verify.
    crate::federation::mount(node.clone()).await;
    crate::control_engine::mount(node.clone()).await;

    // BOOT BRING-UP, native half: respawn every previously-published-and-enabled NATIVE extension, so
    // a published sidecar survives a restart exactly as a wasm one does (issue #64 — the reconcile
    // plan's native actions were computed and then dropped, leaving the child dead and the boot log
    // silent). The node owns the `Launcher`, which is why this half lives here and not in the verb.
    //
    // Placed HERE, after the gateway block, to satisfy BOTH gateway ordering rules (module doc):
    // `install_native` mints each child's `LB_EXT_TOKEN` with `node.key()` and the gateway verifies
    // it with its own key (so the key must be installed first — respawning up beside `load_enabled`
    // would 401 every callback), AND a child that loads its config over a host callback needs the
    // gateway's socket already LISTENING, which is why that block now binds rather than deferring the
    // bind to `serve()`.
    //
    // Across the same workspace set the wasm half used — one node, many tenants, one rule.
    for w in &boot_wss {
        spawn_native_enabled(&node, w).await;
    }
    let agent_server =
        crate::agent::mount(node.clone(), &cfg.agent_model, cfg.agent_caps.clone()).await;

    // LAN discovery: opt-in (`None` ⇒ advertise nothing) and NON-FATAL. A network that refuses
    // mDNS must not take down an otherwise healthy node, so a failure warns and boot continues —
    // the node simply is not discoverable. Advertising is the LAST boot step on purpose: a peer
    // that finds this node should find one that is already serving, not one still coming up.
    let advertised = match cfg.discovery.as_ref().map(lb_discovery::advertise) {
        None => None,
        Some(Ok(handle)) => Some(handle),
        Some(Err(e)) => {
            tracing::warn!(
                error = %e,
                "LAN discovery could not start — the node serves normally but is not discoverable"
            );
            None
        }
    };

    Ok(RunningNode {
        node,
        gateway,
        agent_server,
        advertised,
    })
}

/// Respawn the workspace's enabled native extensions and LOG every one — including the ones that did
/// not come back.
///
/// **Log-and-continue, deliberately** (the open question issue #64 raised). Hard-failing boot on an
/// extension that cannot respawn would turn one broken extension into a node that will not start —
/// and the recovery path for a bad extension (publish a fix, disable it) runs *through the node it
/// just killed, over the gateway that never came up*. That trades a degraded node for an unbootable
/// one and can strand a fleet on an unattended box, which is strictly worse than the failure it
/// guards. Best-effort also matches every neighbouring boot step (`load_enabled`, the seeds, the role
/// mounts).
///
/// What made #64 expensive was never the continuing — it was the SILENCE. So the cure is here: an
/// extension that should be running and is not says so on stderr, every boot, by name and reason. An
/// operator who wants "no degraded boots" can build that on this output; an operator whose node is
/// one broken extension away from unreachable cannot un-build a panic.
async fn spawn_native_enabled(node: &lb_host::Node, ws: &str) {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    match lb_host::spawn_enabled(node, &lb_supervisor::OsLauncher, ws, ts).await {
        Ok(spawned) => {
            for e in &spawned {
                if e.spawned {
                    println!("boot-spawned native extension: {}@{}", e.ext, e.version);
                } else if e.reason != "disabled" && e.reason != "already-running" {
                    // The load-bearing line: an ENABLED native extension boot did not bring back.
                    //
                    // It states what this verb KNOWS (the durable intent said run; boot did not
                    // start it, for this reason) and NOT "it is not running" — an embedder may mount
                    // its own sidecars directly after `boot_full` returns, so a later mount can make
                    // this extension live and that is not a fault. Over-claiming here would send an
                    // operator hunting a healthy extension, which is the opposite of the point.
                    eprintln!(
                        "boot: native extension {}@{} not started by boot bring-up ({}) — it is \
                         installed and enabled; if nothing else starts it, it is not running",
                        e.ext, e.version, e.reason
                    );
                }
            }
        }
        Err(e) => eprintln!("boot native extension respawn for ws={ws} failed: {e}"),
    }
}

#[cfg(test)]
mod printed_url_tests {
    use super::printed_gateway_url_from as url;

    #[test]
    fn the_default_is_unchanged() {
        // Every node that has not opted in must print exactly what it printed
        // before work item 7.
        assert_eq!(url("127.0.0.1:8099", None, None), "http://127.0.0.1:8099");
        assert_eq!(url("0.0.0.0:8099", None, None), "http://0.0.0.0:8099");
    }

    #[test]
    fn a_loopback_bind_is_never_printed_as_https() {
        // The guard rule. The loopback listener is plaintext by design
        // (rubix-fleet docs/rasp-pi/HTTPS.md §3) and no certificate carries a
        // 127.0.0.1 SAN, so https://127.0.0.1:8099 can neither connect nor
        // verify — it is a broken instruction in a journal someone is reading to
        // find out why a box is unreachable.
        for addr in ["127.0.0.1:8099", "localhost:8099", "[::1]:8099"] {
            assert!(
                url(addr, None, Some("https")).starts_with("http://"),
                "{addr} must not be printed as https"
            );
        }
    }

    #[test]
    fn a_non_loopback_bind_honours_the_scheme() {
        assert_eq!(
            url("192.168.1.40:8099", None, Some("https")),
            "https://192.168.1.40:8099"
        );
        assert_eq!(
            url("192.168.1.40:8099", None, Some("HTTPS"))
                .split(':')
                .next(),
            Some("https")
        );
    }

    #[test]
    fn public_url_expresses_the_reverse_proxy_case() {
        // The node binds loopback; operators reach it at a hostname over TLS.
        // Neither the scheme nor the authority of the bind address is what to
        // print, which is why a scheme-only knob cannot express this.
        assert_eq!(
            url("127.0.0.1:8099", Some("https://ai.pi-07.local/"), None),
            "https://ai.pi-07.local"
        );
    }

    #[test]
    fn an_empty_public_url_falls_through_rather_than_printing_nothing() {
        assert_eq!(
            url("127.0.0.1:8099", Some("  "), None),
            "http://127.0.0.1:8099"
        );
    }
}
