# Offline-sync replay readiness barrier — fixing the `applied: 0` flake (session)

- Date: 2026-06-28
- Scope: testing §2.3 (offline/sync) + debugging memory (flaky-bus-timing)
- Stage: S3 sync path / test hardening
- Status: done

## Goal
Kill the flaky `offline_sync_test::offline_edge_writes_apply_idempotently_on_reconnect`, which
intermittently failed `applied: 0` under full parallel `cargo test --workspace` (passed in
isolation). Do it honestly — a real subscription-ready barrier, NOT a sleep or reduced parallelism.

## Root cause
Two stacked races on the replay path (full write-up:
../../debugging/host-tools/offline-sync-replay-races-subscription.md):
1. **Discovery race** — the test used `Node::boot_as` (ambient multicast scouting); under a crowded
   scout domain the edge/hub pair could fail to link, same mechanism as the sibling routed-call
   flake (debugging/bus/routed-call-races-mesh-discovery.md).
2. **Subscription-vs-publish race** — `replay_history` published the instant after the hub called
   `sync_channel`; a Zenoh subscription propagates asynchronously, so the publish landed before the
   hub's interest reached the edge and the messages were dropped (no buffering for an unknown
   subscriber) → hub applied 0 of 3.

## What changed
- **New bus primitive** `rust/crates/bus/src/await_subscriber.rs` — `await_subscriber(bus, ws, rel)`
  declares a publisher and polls Zenoh `Publisher::matching_status()` until a matching subscriber is
  reachable (poll-until-real, no sleep/mock), or a 5s deadline elapses (then falls through — a
  replay to nobody is a harmless idempotent no-op). Exported from `crates/bus/src/lib.rs`. Added
  `tokio.workspace = true` to `crates/bus/Cargo.toml` (for `time::sleep` between polls).
- **`replay_history`** (`rust/crates/host/src/sync.rs`) now awaits that barrier on the channel's
  subscribe key **before** publishing any item. Symmetric/config-free — any node running a replay
  benefits.
- **Test** `rust/crates/host/tests/offline_sync_test.rs` — added `node_on_bus` + `linked_edge_and_hub`
  helpers that stand the pair up with `Bus::peer_with` over a loopback TCP endpoint (mirroring
  `cross_node_routing_test.rs`), so discovery is deterministic. All three tests now use it.

## Decisions & alternatives
- **Barrier on the publisher, not the subscriber.** A subscriber cannot know its interest has
  propagated to all peers; only the publisher can observe matching status. So the readiness check
  belongs in the replay (publish) path. Rejected "make `subscribe` await declaration" — Zenoh gives
  no acknowledged-to-peers signal on the subscriber side; `matching_status` on the publisher is the
  real one.
- **Fall through on deadline rather than error.** A replay that finds no subscriber is legitimate
  (e.g. the workspace-isolation test, where the hub subscribes a different ws). Apply is idempotent,
  so publishing to nobody is safe; erroring would break that path.
- **Both layers kept.** The deterministic link alone wouldn't fix the propagation beat; the barrier
  alone wouldn't fix peers that never link. Each addresses one race.
- **5s deadline.** Generous for real propagation (<1s with a deterministic link, where the loop
  returns instantly); only the genuinely-no-subscriber isolation test pays it.

## Tests
```text
# the flaky assertion, looped
cargo test -p lb-host --test offline_sync_test offline_edge_writes_apply_idempotently_on_reconnect
  → green 20/20
# full file, looped
cargo test -p lb-host --test offline_sync_test  → 3 passed, green 10/10
# sibling (unmodified) still green
cargo test -p lb-host --test cross_node_routing_test  → 3 passed, green 20/20
# whole workspace at default parallelism
cargo test --workspace  → green (no applied: 0)
```
The mandatory categories are still covered in-file: `sync_never_crosses_the_workspace_wall`
(workspace isolation across the sync seam) and the cap-checked reads via `history`. `cargo fmt`
clean.

## Follow-ups
None for this flake. `await_subscriber` is now the reusable readiness primitive for any future
publish-then-expect-apply path.
