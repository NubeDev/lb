//! **A native sidecar spawned during boot must be able to call the host back.** The boot ritual's
//! gateway-readiness contract (node-roles / embed scope).
//!
//! The regression this pins, found live in a downstream product and NOT by any suite: boot bring-up
//! (lb#64) respawns native children inside `boot_full`, and a child that loads its config through a
//! host callback POSTs to `{gateway}/mcp/call` the moment it starts. Two things had to be true and
//! neither was:
//!
//!   1. the child must be TOLD the address — `install_native` read it from a process-global
//!      `LB_GATEWAY_URL` that nothing guarantees is set by spawn time (it was set by a *later*
//!      embedder-side mount, so boot-spawned children got no address at all);
//!   2. the address must be LISTENING — constructing a `Gateway` does not bind. The bind lived in
//!      `RunningNode::serve()`, which the embedder calls AFTER `boot_full` returns, so every
//!      boot-spawned child POSTed into a closed port.
//!
//! Both failures were silent: the child came up with an empty runtime, `GET /extensions` reported
//! `running=true health=ok`, and it did nothing forever (the config load happens exactly once — the
//! race is fatal, not self-healing). A suite that publishes into an already-serving node cannot see
//! this: publishing spawns the child while the gateway is live. Only asserting the state of the world
//! *at the instant `boot_full` returns* catches it.
//!
//! No mocks (CLAUDE §9): a real `boot_full`, a real bound socket, a real TCP connection to it.

use lb_node::{boot_full, BootConfig, GatewayMode};

/// A booted node with the gateway ON, minimal ritual, on an OS-chosen port.
async fn boot_with_gateway() -> lb_node::RunningNode {
    let mut cfg = BootConfig::default();
    cfg.seed_user = None;
    cfg.reactors = false;
    cfg.hello_demo = false;
    cfg.gateway = GatewayMode::Addr("127.0.0.1:0".parse().unwrap());
    boot_full(cfg).await.expect("embedded boot")
}

/// The gateway is **bound and accepting** by the time `boot_full` returns — before any embedder has
/// called `serve()`, and therefore before any boot-spawned child could POST a callback.
///
/// Asserted with a real TCP connect, not by reading a field: "the socket is listening" is precisely
/// the property the child depends on, and it is exactly what constructing a `Gateway` did NOT give.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_gateway_is_listening_before_boot_full_returns() {
    let running = boot_with_gateway().await;
    let (_, listener) = running.gateway.as_ref().expect("gateway on");
    let addr = listener.local_addr().expect("bound to a real port");

    assert_ne!(
        addr.port(),
        0,
        "port 0 must have been RESOLVED by a real bind, not passed through as a request"
    );

    // The load-bearing assertion: a client can connect RIGHT NOW — `serve()` has never been called.
    // Before the fix this connection was refused, which is the exact failure every boot-spawned
    // child hit on its config-load callback.
    tokio::net::TcpStream::connect(addr)
        .await
        .expect("the gateway socket accepts connections as soon as boot_full returns");
}

/// The node knows its OWN callback address, so `install_native` can tell a child where to POST
/// without consulting a process-global `LB_GATEWAY_URL` that nothing guarantees is set.
///
/// It must be the **bound** address: with `127.0.0.1:0` the OS picks the port, so a URL built from
/// the *requested* address would send every child to port 0.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_node_knows_its_own_bound_callback_url() {
    let running = boot_with_gateway().await;
    let (_, listener) = running.gateway.as_ref().expect("gateway on");
    let addr = listener.local_addr().expect("bound");

    let url = running
        .node
        .gateway_url()
        .expect("the boot layer installed the node's own callback URL");
    assert_eq!(
        url,
        format!("http://{addr}"),
        "the callback URL must be the REAL bound address (port 0 resolved), not the requested one"
    );
}

/// A headless node installs no callback URL — there is no gateway to call back to. Children spawn
/// with no address and their callback client fails cleanly, exactly as before (unchanged posture).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_headless_node_has_no_callback_url() {
    let mut cfg = BootConfig::default();
    cfg.seed_user = None;
    cfg.reactors = false;
    cfg.hello_demo = false;
    cfg.gateway = GatewayMode::Off;
    let running = boot_full(cfg).await.expect("headless boot");

    assert!(running.gateway.is_none());
    assert!(
        running.node.gateway_url().is_none(),
        "a node with no gateway must not hand children an address to POST into"
    );
}
