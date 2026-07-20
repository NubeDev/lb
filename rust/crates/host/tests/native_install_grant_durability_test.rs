//! Native-install grant durability regression (datasources scope). The bug: two writers touched the
//! same `Install.granted` set and the boot writer clobbered the runtime one.
//!
//!   - RUNTIME: `datasource.add` self-approves an endpoint — `federation::net::grant_endpoint`
//!     APPENDS `net:tls:{host}:{port}:connect` so a source added from the UI connects with no boot
//!     env var / restart (proven by `datasource_crud_ownership_test::add_self_approves_a_new_endpoint`).
//!   - BOOT: `install_native` RECOMPUTES `requested ∩ admin_approved` and OVERWRITES the record.
//!
//! Boot re-installs the sidecar on every start, so the recompute dropped every endpoint the admin had
//! approved from the UI. The source record and its DSN secret survived — only the `net:*` grant
//! vanished — so the next `datasource.test`/`federation.query` was refused pre-connect by
//! `enforce_endpoint` and surfaced as an opaque `denied`, which reads like a database/credential
//! failure and is not one. Every UI-registered datasource worked until the first restart, then died.
//!
//! The existing self-approval test wrote the install record directly and never re-installed, so the
//! append was covered but its SURVIVAL across a re-install was not — this file closes that seam by
//! driving the real `install_native` twice against the real embedded store.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use lb_assets::read_install;
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{install_native, Node};
use lb_supervisor::{
    read_frame, write_frame, Channel, Kill, Launcher, Method, Reply, Request, SupervisorError,
};
use tokio::io::duplex;

const MANIFEST: &str = include_str!("../../../extensions/echo-sidecar/extension.toml");

struct FakeLauncher;
struct NoKill;
impl Kill for NoKill {
    fn kill(self: Box<Self>) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(async {})
    }
}
impl Launcher for FakeLauncher {
    async fn launch(
        &self,
        _exec: &str,
        _args: &[String],
        _env: &HashMap<String, String>,
    ) -> Result<Channel, SupervisorError> {
        let (host_side, child_side) = duplex(8192);
        let (mut cr, mut cw) = tokio::io::split(child_side);
        tokio::spawn(async move {
            while let Ok(body) = read_frame(&mut cr).await {
                let req: Request = serde_json::from_slice(&body).unwrap();
                let reply = match req.method {
                    Method::Init => Reply::ok(req.id, "ready"),
                    Method::Health => Reply::ok(req.id, "ok"),
                    Method::Call => Reply::ok(req.id, "{}"),
                    Method::Shutdown => Reply::ok(req.id, "bye"),
                };
                if write_frame(&mut cw, &serde_json::to_vec(&reply).unwrap())
                    .await
                    .is_err()
                {
                    break;
                }
                if req.method == Method::Shutdown {
                    break;
                }
            }
        });
        let (read, write) = tokio::io::split(host_side);
        Ok(Channel {
            write: Box::pin(write),
            read: Box::pin(read),
            kill: Box::new(NoKill),
        })
    }
}

fn admin(ws: &str) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:ada".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: vec!["mcp:native.install:call".into()],
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).expect("freshly minted token verifies")
}

/// Read the extension id the bundled test manifest declares, so the assertions follow the manifest
/// rather than hardcoding an id (this file names no product extension — §10).
fn ext_id() -> String {
    MANIFEST
        .lines()
        .map(str::trim)
        .find_map(|l| l.strip_prefix("id"))
        .and_then(|rest| rest.trim_start().strip_prefix('='))
        .expect("manifest declares an id")
        .trim()
        .trim_matches('"')
        .to_string()
}

/// THE REGRESSION: an endpoint approved at runtime must survive the next boot's re-install.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_runtime_approved_endpoint_survives_reinstall() {
    let node = Node::boot().await.unwrap();
    let ws = "acme";
    let ada = admin(ws);
    let id = ext_id();

    // Boot 1: the fresh-node state. (The bundled test manifest requests no caps, so the
    // `requested ∩ approved` recompute is empty here regardless of what the binary approves — which
    // is precisely what made the clobber total: the re-install wrote an EMPTY grant set.)
    let approved: Vec<String> = vec![];
    install_native(
        &node,
        &FakeLauncher,
        &ada,
        ws,
        MANIFEST,
        "target/debug",
        &approved,
        1,
    )
    .await
    .expect("first install");

    // RUNTIME: an admin registers a source at a new endpoint, which self-approves it by appending
    // to the persisted grant (what `grant_endpoint` does on the `datasource.add` path).
    let runtime_grant = "net:tls:timescale.example.com:5434:connect".to_string();
    let mut rec = read_install(&node.store, ws, &id).await.unwrap().unwrap();
    rec.granted.push(runtime_grant.clone());
    lb_assets::record_install(&node.store, ws, &rec)
        .await
        .unwrap();

    // Boot 2: the SAME install runs again with the SAME approved set — a plain node restart.
    install_native(
        &node,
        &FakeLauncher,
        &ada,
        ws,
        MANIFEST,
        "target/debug",
        &approved,
        2,
    )
    .await
    .expect("re-install");

    let after = read_install(&node.store, ws, &id).await.unwrap().unwrap();
    assert!(
        after.granted.contains(&runtime_grant),
        "a runtime-approved endpoint must survive re-install (this is the bug: the recompute \
         dropped it, and the next connect was refused as an opaque `denied`): {:?}",
        after.granted
    );
}

/// The carry-forward must not GROW the record: once an endpoint is carried, further boots keep
/// exactly one copy. (A runtime-appended grant is the only `net:*` source here — the bundled test
/// manifest requests no caps, so the `requested ∩ approved` recompute is empty by construction.)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repeated_reinstalls_do_not_duplicate_grants() {
    let node = Node::boot().await.unwrap();
    let ws = "acme";
    let ada = admin(ws);
    let id = ext_id();
    let runtime_grant = "net:tls:timescale.example.com:5434:connect".to_string();

    install_native(
        &node,
        &FakeLauncher,
        &ada,
        ws,
        MANIFEST,
        "target/debug",
        &[],
        1,
    )
    .await
    .expect("first install");

    // The runtime self-approval, appended once.
    let mut rec = read_install(&node.store, ws, &id).await.unwrap().unwrap();
    rec.granted.push(runtime_grant.clone());
    lb_assets::record_install(&node.store, ws, &rec)
        .await
        .unwrap();

    // Two more boots: the grant is carried each time, but must never accumulate.
    for ts in 2..=3 {
        install_native(
            &node,
            &FakeLauncher,
            &ada,
            ws,
            MANIFEST,
            "target/debug",
            &[],
            ts,
        )
        .await
        .expect("re-install");
    }

    let after = read_install(&node.store, ws, &id).await.unwrap().unwrap();
    let count = after.granted.iter().filter(|g| **g == runtime_grant).count();
    assert_eq!(count, 1, "grants must not accumulate: {:?}", after.granted);
}

/// The wall must NOT widen: a non-`net` cap the admin un-approves still disappears on re-install.
/// Only the runtime-appended `net:*` surface is carried — revocation keeps working everywhere else.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn revoking_a_non_net_cap_still_takes_effect() {
    let node = Node::boot().await.unwrap();
    let ws = "acme";
    let ada = admin(ws);
    let id = ext_id();

    // Boot 1 approves a secret cap; boot 2 does not.
    install_native(
        &node,
        &FakeLauncher,
        &ada,
        ws,
        MANIFEST,
        "target/debug",
        &["secret:federation/*:get".to_string()],
        1,
    )
    .await
    .expect("first install");

    install_native(
        &node,
        &FakeLauncher,
        &ada,
        ws,
        MANIFEST,
        "target/debug",
        &[],
        2,
    )
    .await
    .expect("re-install with the cap un-approved");

    let after = read_install(&node.store, ws, &id).await.unwrap().unwrap();
    assert!(
        !after.granted.iter().any(|g| g.starts_with("secret:")),
        "an un-approved non-net cap must be revoked by the recompute: {:?}",
        after.granted
    );
}
