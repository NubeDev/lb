# `offline_sync_test` flakes `applied: 0` — replay publishes before the hub's subscription is live

- Area: host-tools (sync path) / bus
- Status: resolved
- First seen: 2026-06-28
- Resolved: 2026-06-28
- Session: ../../sessions/host-tools/offline-sync-readiness-session.md
- Regression test: rust/crates/host/tests/offline_sync_test.rs
  (`offline_edge_writes_apply_idempotently_on_reconnect`)

## Symptom

Under a full parallel `cargo test --workspace` the offline-sync reconnect test intermittently
failed with the hub applying **none** of the three offline writes:

```text
thread 'offline_edge_writes_apply_idempotently_on_reconnect' panicked at
crates/host/tests/offline_sync_test.rs:93:
assertion `left == right` failed: hub applies all three offline writes on reconnect
  left: 0
 right: 3
```

It passed 5/5 in isolation and only flaked under concurrent test-binary load — a classic timing
race, not a logic bug.

## Reproduce

`cargo test --workspace` at default parallelism (intermittent — depends on which Zenoh-heavy test
binaries are co-running). The window widens on a CPU-saturated box.

## Investigation

Two independent races stacked on the replay path (`replay_history` publishes; the hub's
`sync_channel` subscribes; `drain` applies):

1. **Discovery race.** The test stood its edge + hub up with `Node::boot_as`, which opens
   `Bus::peer()` — i.e. **ambient multicast scouting only**. Under a full workspace run, hundreds
   of in-process Zenoh peers share one scout domain and gossip between a *specific* pair can stall
   indefinitely (the exact mechanism documented for the sibling routed-call flake,
   [../bus/routed-call-races-mesh-discovery.md](../bus/routed-call-races-mesh-discovery.md)). If the
   two peers never link, nothing replayed can ever reach the hub.
2. **Subscription-vs-publish race.** Even with a live link, Zenoh pub/sub is fire-and-forget and a
   subscriber declared on one peer propagates to its peers **asynchronously**. `replay_history`
   `put`s its items the instant after the hub calls `sync_channel`, so the publish can land *before*
   the hub's interest has reached the edge — Zenoh does not buffer for a subscriber it doesn't yet
   know about, so those messages are dropped on the floor and the hub applies 0.

## Root cause

The replay path assumed two things it never guaranteed: that the peers were linked, and that a
publish issued right after a `subscribe` would be observed. Both are false under load. The publisher
is the only party that can *observe* whether a matching subscriber is reachable (via Zenoh
`matching_status`); a subscriber cannot know its interest has propagated everywhere. So the honest
barrier belongs on the publisher.

## Fix

Two layers, mirroring the proven routed-call fix:

1. **Subscription-readiness barrier (production fix).** New reusable bus primitive
   `rust/crates/bus/src/await_subscriber.rs`: `await_subscriber(bus, ws, rel)` declares a publisher
   for the key and polls `Publisher::matching_status()` until a matching subscriber is reachable
   (poll-until-real, no sleep-masking, no mock), or a deadline elapses (then it falls through — a
   replay to nobody is a harmless no-op because apply is idempotent). `replay_history`
   (`rust/crates/host/src/sync.rs`) now awaits this on the channel's subscribe key **before** it
   publishes any item. This is symmetric/config-free — any node running a replay benefits, edge or
   hub.
2. **Deterministic link (test fix).** `offline_sync_test.rs` now stands its pair up with
   `Bus::peer_with` over a loopback TCP endpoint (`linked_edge_and_hub`), exactly like
   `cross_node_routing_test.rs`, so discovery is deterministic regardless of scout-domain noise.
   Without this the barrier above would just hit its deadline (no subscriber ever reachable) and the
   flake would persist as a timeout instead of `applied: 0`.

Both are needed: layer 2 guarantees the peers *can* see each other; layer 1 guarantees the publish
waits until they *do*.

## Verification

- `cargo test -p lb-host --test offline_sync_test offline_edge_writes_apply_idempotently_on_reconnect`
  — green **20×** in a row.
- `cargo test -p lb-host --test offline_sync_test` (full file) — green **10×** in a row.
- `cargo test --workspace` at default parallelism — green (no `applied: 0`).

## Prevention

- `await_subscriber` is the reusable readiness primitive for any future publish-then-expect-apply
  path (replay, catch-up, hand-off) — reach for it instead of a blind sleep.
- The regression test links its peers deterministically, so the discovery race cannot recur for it.
- Related: [../bus/routed-call-races-mesh-discovery.md](../bus/routed-call-races-mesh-discovery.md)
  (sibling discovery race + `Bus::peer_with`), [../bus/in-process-peers-share-the-keyspace.md](../bus/in-process-peers-share-the-keyspace.md)
  (unique-ws isolation), [../bus/zenoh-needs-multi-thread-runtime.md](../bus/zenoh-needs-multi-thread-runtime.md).
