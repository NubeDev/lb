# ROS driver — Slice 1: vendored async client + sidecar skeleton

Status: **done** (build/test/fmt green). Co-located session log (this is 100% an extension —
nothing in the repo-root `docs/` tree).

## Goal (from the build plan)

Slice 1 = client + skeleton:
- Vendor `rust-ros` (ported to async `reqwest`, `sqlx`/Postgres dropped) into `src/ros_client/`.
- Add `rust/extensions/ros/Cargo.toml` (a `ros-sidecar` bin) and add `extensions/ros` to the
  workspace members.
- `src/ros_api.rs`: the `RosApi` trait (the ONE external-fake seam) + the real `rust-ros`-backed
  impl + `src/ros_fake.rs` (test double serving a canned tree, accepting writes).
- Stub sidecar `main.rs` (mirror `fleet-monitor`) that starts and serves an empty MCP tool set.
- `cargo build --workspace` and `cargo fmt` green.

## What shipped

- `src/ros_client/` — the vendored client, **async**:
  - `client.rs`: `reqwest::Client` (not `blocking`), async `get_json`/`patch_json`; the
    `External {token}` auth header preserved. `sqlx`/`tokio-full`/`chrono` deps dropped.
  - `networks.rs` / `devices.rs` / `points.rs` / `system.rs` / `histories.rs` / `users.rs`:
    models verbatim from the source; every `impl Client` call is now `async fn … .await`.
  - `points.rs` gained `Priority::set_slot(slot, value)` — the `{slot, value|null}` write
    ergonomics (ros-scope resolved decision) mapped onto the 16-slot array, with an
    out-of-range slot rejected before any REST call.
- `src/ros_api.rs` — the `RosApi` trait (the single external-fake seam, testing-scope §0): `ping`,
  `list_networks(with_tree)`, `list_devices`, `list_points`, `get_point`, `write_point_slot`.
  `RealRosApi` is the `rust-ros`-backed impl (one async `Client` per connection). `RosApiError`
  distinguishes `Unreachable` (poll-backoff signal) / `NotFound` / `Api` / `InvalidInput`. Never
  carries the token.
- `src/ros_fake.rs` — `RosFake`: canned network→device→point tree, records setpoint writes
  (`writes()`), an `unreachable` toggle for the backoff/retry paths, `set_value` to simulate the
  physical value changing between ticks. The ONE allowed fake, behind the ONE trait.
- `src/main.rs` — the `ros-sidecar` bin: mirrors `fleet-monitor` — `lb-supervisor` framed stdio
  loop (`init`/`health`/`call`/`shutdown`), injected `LB_EXT_WS`/`LB_EXT_ID`. Stateless.
- `src/call.rs` — tool dispatch; **empty tool set** this slice (unknown-tool → explicit error).
- `Cargo.toml` — the `ros-sidecar` bin crate; `extensions/ros` added to `rust/Cargo.toml` members.

## Decisions

- **Async over `spawn_blocking`** (open question #1): ported the client to async `reqwest`. The
  poller will run many concurrent reads; a blocking client on the async runtime would stall the
  reactor. Cleaner poller, no thread-pool juggling.
- **`RosApi` is ROS-shaped, not a generic "driver" trait.** The *reusable* seam is the poller's
  `Source` trait (slice 3); `RosSource` will adapt a `RosApi` to it. That keeps ROS vocabulary in
  this one file and out of the reusable engine (ros-scope: "not one line of ROS vocabulary enters a
  core crate" — and the engine stays driver-agnostic).
- **`allow(dead_code)` on the vendored client + the seam.** The client is a faithful, complete copy
  of the box's REST surface; the trait/impl land now and get their first callers in slices 2–4.
  Scoped module-level allows with a comment, not a blanket crate allow.

## Tests (this slice)

- `call::tests::unknown_tool_is_an_explicit_error` — an unwired tool is an error reply, never a
  silent ok.
- `call::tests::bad_params_is_an_error_not_a_panic`.

The mandatory capability-deny / workspace-isolation / poller-gating / point.write→outbox tests
arrive with the slices that introduce those paths (2–4), against the real store/bus/ingest/outbox/
gateway with only the box faked.

## Green output

```
cargo build -p ros-sidecar   → Finished (0 warnings)
cargo test  -p ros-sidecar   → test result: ok. 2 passed; 0 failed
cargo fmt --check -p ros-sidecar → clean
cargo build --workspace      → Finished (member addition does not break the tree)
```

## Next: Slice 2 — CRUD tree

One file per verb under `src/handlers/`: `ros|network|device|point . list|get|create|update|delete`
+ `ros.ping`. Each workspace-resolve-first, then its own capability check, then proxies `RosApi`.
Keyset-paged `{items, next_cursor}` envelope on `list`. Minimal config shadow
(`{uuid, name, enable, poll_rate, parent}`) in the store; token stashed via `lb-secrets`, never
returned by `get`/`list`, never logged.
