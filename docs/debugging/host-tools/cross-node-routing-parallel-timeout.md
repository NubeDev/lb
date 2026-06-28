# `cargo test --workspace` timed out in cross-node routing during host-tools verification

- Area: host-tools
- Status: resolved — root cause found and fixed under [../bus/routed-call-races-mesh-discovery.md](../bus/routed-call-races-mesh-discovery.md)
- First seen: 2026-06-28
- Resolved: 2026-06-28
- Session: ../../sessions/host-tools/host-tools-session.md
- Regression test: rust/crates/host/tests/cross_node_routing_test.rs (now links its peers deterministically)

## Symptom
During host-tools verification, the default parallel `cargo test --workspace` failed twice in the
pre-existing `lb-host` cross-node routing test:

```text
thread 'a_call_on_the_edge_routes_to_the_extension_on_the_hub' panicked at
crates/host/tests/cross_node_routing_test.rs:88:6:
a routed call returns in time: Elapsed(())
```

The new `host_tools_test` suite passed before this failure, and the failing cross-node test passed when
run isolated.

## Reproduce
Run:

```text
cd rust
cargo test --workspace
```

Observed twice on 2026-06-28 while another AI session was also running in the repo.

## Investigation
- `cargo test -p lb-host --test host_tools_test` passed.
- `cargo test -p lb-host --test cross_node_routing_test a_call_on_the_edge_routes_to_the_extension_on_the_hub`
  passed isolated.
- `cargo test -p lb-host --test cross_node_routing_test -- --test-threads=1` passed.
- `cargo test --workspace -- --test-threads=1` passed.

## Root cause
**Resolved in a follow-up** — see [../bus/routed-call-races-mesh-discovery.md](../bus/routed-call-races-mesh-discovery.md).
The test linked its two in-process peers via best-effort multicast scouting, which under a crowded
scout domain (a full parallel workspace run) could stall past any timeout — a discovery race, not the
host-tools implementation (which touches no Zenoh routing code). The original instinct here was right.

The fix links the pair over an explicit loopback TCP endpoint (`Bus::peer_with`) so discovery is
deterministic; the cross-node test now passes at default parallelism without `--test-threads=1`.

## Fix
No production or routing-code fix was made in this session. The verification workaround was to run the
workspace with serial test threads.

## Verification
`cargo test --workspace -- --test-threads=1` passed end to end.

## Prevention
Future follow-up should make `cross_node_routing_test` deterministic under default parallel
`cargo test --workspace`, likely by isolating Zenoh sessions/test keys or serializing that file's tests.
