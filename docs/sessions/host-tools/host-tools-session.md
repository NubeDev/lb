# Host tools — built-in node introspection verbs (session)

- Date: 2026-06-28
- Scope: ../../scope/host-tools/host-tools-scope.md
- Stage: S10 — platform retrofit / host dispatch surface
- Status: done

## Goal
Ship the backend `host.*` MCP family end to end through the existing host-native tool chokepoint:
`host.net.info`, `host.net.reach`, `host.time.now`, `host.time.zones`, `host.fs.stat`, and
`host.fs.list`. The acceptance gate is the scope's MCP surface: per-verb capability denies,
workspace-first denial for wrong-workspace callers, bounded network probes, cross-platform identical DTO
shape, and no file contents/secrets in results.

## What changed
- Added `rust/crates/host/src/host_tools/` with one file per verb:
  `net/info.rs`, `net/reach.rs`, `time/now.rs`, `time/zones.rs`, `fs/stat.rs`, and `fs/list.rs`.
  `net/platform.rs` owns interface/hostname platform facts; `fs/path.rs` owns path normalization.
- Wired one `host.` prefix into `rust/crates/host/src/tool_call.rs`, delegating to
  `call_host_tool` exactly like the existing `agent.` branch.
- Exported the host-tools module from `rust/crates/host/src/lib.rs` for tests and downstream callers.
- Added workspace dependencies for `if-addrs`, `hostname`, `chrono`, `chrono-tz`, and
  `iana-time-zone`; recorded them in `../../key-stack.md`.
- Added `rust/crates/host/tests/host_tools_test.rs` covering the mandatory categories.

## Decisions & alternatives
- Chose `if-addrs` + `hostname` for network identity and `chrono` + `chrono-tz` +
  `iana-time-zone` for time facts. `cargo check -p lb-host` passed under the repo's
  `rust/.cargo/config.toml` zig/no-system-cc setup.
- Kept v1 backend/agent-facing only. There is no dedicated UI panel; generic MCP callers can already
  reach these verbs when granted.
- Kept `host.net.reach` TCP-only. UDP was rejected because it is not a clear one-shot reachability
  fact; the verb is a bounded TCP connect probe, not a client/socket API.
- Used deterministic `host.fs.list` ordering: one level, name-ascending, 1000-entry cap, with
  `truncated: true` when capped.

## Tests
Focused green run:

```text
$ cargo test -p lb-host --test host_tools_test
   Compiling lb-host v0.1.0 (/home/user/code/rust/lb/rust/crates/host)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.94s
     Running tests/host_tools_test.rs (target/debug/deps/host_tools_test-e53237857363fe13)

running 10 tests
test fs_stat_without_its_cap_is_denied ... ok
test time_zones_without_its_cap_is_denied ... ok
test dto_allow_lists_leak_no_file_contents_or_extra_network_fields ... ok
test every_verb_returns_the_same_shape_on_this_os ... ok
test other_workspace_token_is_denied_before_node_global_fact_is_read ... ok
test fs_list_without_its_cap_is_denied ... ok
test time_now_without_its_cap_is_denied ... ok
test net_info_without_its_cap_is_denied ... ok
test reach_uses_a_bounded_tcp_probe_and_rejects_port_ranges ... ok
test net_reach_without_its_cap_is_denied ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.71s
```

Mandatory categories covered: six capability-deny tests, other/empty-workspace gate denial,
cross-platform DTO shape allow-lists, bounded `host.net.reach`, real temp-dir filesystem facts, real
loopback listener for reachability, and leak-nothing DTO allow-lists.

Format and workspace gate:

```text
$ cargo fmt --check
```

No output; command exited 0 after the final patch.

The exact default workspace command was attempted twice and failed in an existing cross-node routing
timeout unrelated to host-tools:

```text
$ cargo test --workspace
test a_call_on_the_edge_routes_to_the_extension_on_the_hub ... FAILED

failures:

---- a_call_on_the_edge_routes_to_the_extension_on_the_hub stdout ----
thread 'a_call_on_the_edge_routes_to_the_extension_on_the_hub' (...) panicked at crates/host/tests/cross_node_routing_test.rs:88:6:
a routed call returns in time: Elapsed(())

test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 26.57s
error: test failed, to rerun pass `-p lb-host --test cross_node_routing_test`
```

The failing pre-existing cross-node test passed isolated and serially:

```text
$ cargo test -p lb-host --test cross_node_routing_test -- --test-threads=1
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.22s
     Running tests/cross_node_routing_test.rs (target/debug/deps/cross_node_routing_test-a2b6a2afe36cbe63)

running 3 tests
test a_call_on_the_edge_routes_to_the_extension_on_the_hub ... ok
test a_principal_in_ws_b_cannot_route_into_ws_a ... ok
test a_routed_call_is_denied_without_the_grant_and_never_leaves_the_edge ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 65.56s
```

Full workspace green under serial test threads before the final narrow hardening patch (`port = 0`
rejection and symlink-preserving directory entry metadata):

```text
$ cargo test --workspace -- --test-threads=1
...
     Running tests/host_tools_test.rs (target/debug/deps/host_tools_test-34f2bd0ce3d23d56)

running 10 tests
test dto_allow_lists_leak_no_file_contents_or_extra_network_fields ... ok
test every_verb_returns_the_same_shape_on_this_os ... ok
test fs_list_without_its_cap_is_denied ... ok
test fs_stat_without_its_cap_is_denied ... ok
test net_info_without_its_cap_is_denied ... ok
test net_reach_without_its_cap_is_denied ... ok
test other_workspace_token_is_denied_before_node_global_fact_is_read ... ok
test reach_uses_a_bounded_tcp_probe_and_rejects_port_ranges ... ok
test time_now_without_its_cap_is_denied ... ok
test time_zones_without_its_cap_is_denied ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.64s
...
   Doc-tests lb_sync

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests lb_tags

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

After the final hardening patch, the directly affected suite was rerun and passed:

```text
$ cargo test -p lb-host --test host_tools_test
   Compiling lb-host v0.1.0 (/home/user/code/rust/lb/rust/crates/host)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.65s
     Running tests/host_tools_test.rs (target/debug/deps/host_tools_test-e53237857363fe13)

running 10 tests
test time_now_without_its_cap_is_denied ... ok
test net_info_without_its_cap_is_denied ... ok
test fs_stat_without_its_cap_is_denied ... ok
test reach_uses_a_bounded_tcp_probe_and_rejects_port_ranges ... ok
test every_verb_returns_the_same_shape_on_this_os ... ok
test net_reach_without_its_cap_is_denied ... ok
test other_workspace_token_is_denied_before_node_global_fact_is_read ... ok
test dto_allow_lists_leak_no_file_contents_or_extra_network_fields ... ok
test time_zones_without_its_cap_is_denied ... ok
test fs_list_without_its_cap_is_denied ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.25s
```

A final post-hardening full serial workspace rerun was started, reached and passed
`host_tools_test`, and was then stopped at the user's request because it was spending time in unrelated
workspace suites.

## Debugging
- `../../debugging/host-tools/cross-node-routing-parallel-timeout.md` records the unrelated default
  `cargo test --workspace` cross-node timeout seen during verification. Host-tools tests passed; the
  failing cross-node test passed isolated/serially; full workspace passed with `--test-threads=1`.

## Public / scope updates
- Promoted durable truth to `../../public/host-tools/host-tools.md`.
- Added the shipped summary to `../../public/SCOPE.md`.
- Resolved all scope open questions in `../../scope/host-tools/host-tools-scope.md`.
- Added the shipped slice row to `../../STATUS.md`.

## Dead ends / surprises
- The implementation needs a minimal `rust/crates/host/src/lib.rs` module/export edit in addition to
  the originally listed `host_tools/**` and `tool_call.rs`; without that the new module is never
  compiled or reachable from the host bridge/tests.
- The crate choices add five Cargo.lock entries (`if-addrs`, `hostname`, `chrono-tz`, `phf`,
  `phf_shared`) and reuse already-transitive `chrono`/`iana-time-zone`.

## Follow-ups
- STATUS.md updated? Yes.
- Follow up on the pre-existing parallel cross-node routing timeout if default workspace tests must be
  green under concurrent sessions.

## Follow-up (2026-06-28): cross-node routing flake — root-caused and fixed
The "pre-existing parallel cross-node routing timeout" above was chased to root cause and fixed.
- **Root cause:** `cross_node_routing_test` linked its two in-process Zenoh peers via best-effort
  multicast scouting. Under a crowded scout domain (full parallel `cargo test --workspace`, hundreds
  of peers) that discovery could stall past any timeout — a retry-to-30s loop still failed because the
  peers *never discovered each other*. A `query`/`get` issued before the hub's queryable is known
  blocks until Zenoh's ~10s query timeout, tripping the test's wrapper.
- **Fix (test layer + tiny bus seam):** added `Bus::peer_with(listen, connect)` (explicit endpoints —
  the production-faithful, config-only posture per README §3.1/§6.2, symmetric, no `if cloud`). The
  test now picks a free loopback port, the hub listens on it and the edge connects to exactly it —
  deterministic point-to-point discovery. A `route_until_reachable` poll-until-Ok barrier absorbs the
  residual queryable-propagation beat. No mocks; real Zenoh TCP peers.
- **Proof:** isolated 5×, under CPU saturation 5×, and `cargo test -p lb-host` (the Zenoh-heaviest
  crate) at default parallelism — all green. A full `cargo test --workspace` ran clean once; a later
  run failed to *compile* `lb-role-gateway` (unrelated concurrent edit in `role/gateway/`), not a flake.
- Debug entry: `../../debugging/bus/routed-call-races-mesh-discovery.md` (supersedes the host-tools
  investigating note).
