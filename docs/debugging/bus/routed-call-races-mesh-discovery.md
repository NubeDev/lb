# A routed cross-node tool call times out — the two in-process peers race mesh discovery

- Area: bus
- Status: resolved
- First seen: 2026-06-28
- Resolved: 2026-06-28
- Session: ../../sessions/host-tools/host-tools-session.md
- Regression test: rust/crates/host/tests/cross_node_routing_test.rs
  (`a_call_on_the_edge_routes_to_the_extension_on_the_hub`)

## Symptom

`cross_node_routing_test::a_call_on_the_edge_routes_to_the_extension_on_the_hub` timed out:

```text
thread 'a_call_on_the_edge_routes_to_the_extension_on_the_hub' panicked at
crates/host/tests/cross_node_routing_test.rs:88:6:
a routed call returns in time: Elapsed(())
```

It failed under a full parallel `cargo test --workspace` (and was reproduced on a clean HEAD
worktree, so it was a pre-existing flake, not a regression). The other two tests in the file
(`…_denied_without_the_grant…`, `…ws_b_cannot_route_into_ws_a`) always passed — they assert a
*deny* that fires on the edge **before** any bus hop, so they never depend on discovery.

## Reproduce

`cargo test --workspace` at default parallelism (intermittent — depends on which test binaries are
running concurrently when this one executes). Reliable repro: run the test on a CPU-saturated box —
under load, ambient multicast convergence between the pair routinely exceeds the old fixed timeout.

## Investigation

- Isolated, the test passed every time; the flake only appeared under load / full-workspace runs.
  Classic discovery race, not a logic bug.
- Traced the routed path: `dispatch::route` → `lb_bus::query` → `session.get(key)`. When the edge's
  peer has **not yet discovered** the hub's queryable, the `get` reaches no responder and its reply
  channel blocks until Zenoh's default ~10s query timeout. A queryable that joins *after* the `get`
  is issued does not retroactively answer it — so the in-flight call just blocks, and the test's 5s
  `tokio::time::timeout` wrapper fires first → `Elapsed`.
- **Ruled out "timeout too tight":** rewrote the test to retry the real call for up to 30s. It STILL
  failed under a full `cargo test --workspace`, with the last attempt reporting "queryable not yet
  reachable" for the entire 30s — i.e. the two peers *never discovered each other*. More time does
  not fix a discovery that isn't completing.
- Root of *that*: both peers opened with `zenoh::Config::default()`, i.e. **ambient multicast
  scouting only**. Under a full workspace run, hundreds of in-process Zenoh peers (every node-booting
  test binary, ×2 peers each — see cargo-test-workspace-ooms-with-many-peers.md) share one multicast
  scout domain, and gossip between a *specific* pair can stall indefinitely. `agent_routed_test`
  (same edge+hub shape) passed in the same run purely because its pair got luckier with timing.

## Root cause

The test relied on **best-effort multicast scouting** to link its two peers. That is non-deterministic
and degrades badly under a crowded scout domain — so a routed call could be issued before (or without)
the edge ever learning the hub's queryable, and block past any timeout. Production `call`/`query` are
correct: when nothing is reachable they return "no node answered"; the defect was the test assuming
ambient mesh convergence it never guaranteed.

## Fix

Two layers, in `rust/crates/host/tests/cross_node_routing_test.rs` (a test-layer fix, like
in-process-peers-share-the-keyspace.md — production routing was correct):

1. **Deterministic link (primary).** Stop relying on multicast. The test picks a free loopback port,
   the hub **listens** on `tcp/127.0.0.1:<port>` and the edge **connects** to exactly it — a
   point-to-point link that forms in milliseconds, independent of the scout domain. This needed a
   small, production-faithful config seam on the bus:
   - `rust/crates/bus/src/peer.rs`: `Bus::peer_with(listen, connect)` opens a peer with explicit
     `listen`/`connect` endpoints via `Config::insert_json5`. This is exactly the posture the
     deployment layer wires for real multi-node (README §3.1/§6.2: peer/router mode and upstream
     endpoints are *config*, never a code branch) — so it is symmetric, not an `if cloud`.
   - The test builds its two `Node`s directly on those buses (`node_on_bus`), mirroring the existing
     direct-construction pattern in `ext_publish_test.rs`. Nothing mocked — real Zenoh TCP peers.
   - (We choose the port ourselves with a throwaway `TcpListener` bind rather than reading it back
     from Zenoh, because `Session::info().locators()` is behind zenoh's `unstable` feature, which we
     did not want to enable workspace-wide.)
2. **Readiness barrier (belt-and-suspenders).** `route_until_reachable` retries the real routed call
   until the first `Ok` (deadline 20s, but it returns the instant a call succeeds — <1s with the
   direct link). This absorbs the residual beat between the link forming and the hub's queryable
   *declaration* propagating to the edge, deterministically (poll-until-reachable, not a blind sleep).

## Verification

- `cargo test -p lb-host --test cross_node_routing_test` — green **5×** in a row in isolation
  (~1.6s each).
- Under deliberate CPU saturation (`yes` on 2× cores) — the load that broke the old multicast
  version — green **5×** in a row (10–18s each, dominated by starved node boot, not routing).
- `cargo test -p lb-host` (the whole Zenoh-heaviest crate: cross_node, agent_routed, offline_sync,
  messaging, presence, hot_reload, …) at **default parallelism** — exit 0, no failures.
- `cargo test --workspace` at default parallelism — the previously-flaky `Elapsed` is gone (a fully
  green run captured). NOTE: a later workspace run failed to **compile** `lb-role-gateway`
  (`system_tools`/`system_acp` unresolved in `role/gateway/src/server.rs`) — an unrelated in-progress
  edit by a concurrent session in `role/gateway/`, not this change and not a test flake.

## Prevention

- The regression test now links its peers deterministically, so the discovery race cannot recur for
  it regardless of workspace concurrency.
- `Bus::peer_with` is the reusable, production-faithful primitive for any future test (or wiring
  layer) that needs a deterministic point-to-point link instead of ambient scouting.
- Related: [in-process-peers-share-the-keyspace](in-process-peers-share-the-keyspace.md) (unique-ws
  isolation — also a test-layer correction), [zenoh-needs-multi-thread-runtime](zenoh-needs-multi-thread-runtime.md),
  [cargo-test-workspace-ooms-with-many-peers](cargo-test-workspace-ooms-with-many-peers.md) (why the
  scout domain gets so crowded). Supersedes the earlier investigating note
  [../host-tools/cross-node-routing-parallel-timeout.md](../host-tools/cross-node-routing-parallel-timeout.md).
