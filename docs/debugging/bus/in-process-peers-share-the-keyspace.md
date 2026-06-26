# A live subscriber receives a message published by a *different* test

- Area: bus
- Status: resolved
- First seen: 2026-06-26
- Resolved: 2026-06-26
- Session: ../../sessions/bus/messaging-session.md
- Regression test: rust/crates/host/tests/messaging_test.rs + messaging_isolation_test.rs (unique-ws fixture)

## Symptom

`posted_message_appears_to_a_live_subscriber` passed when run alone but FAILED when run in the
same binary as the other messaging tests:

```
assertion `left == right` failed
  left: "dup"      # an id posted by re_posting_the_same_id_is_idempotent
 right: "m1"        # the id this test posted
```

The subscriber received a message that a *concurrently running test* published.

## Reproduce

`cargo test -p lb-host --test messaging_test` (all three tests, default concurrency). Passes
with `posted_message_appears_to_a_live_subscriber` run alone; fails when the suite runs
together. Every test used the same workspace `"acme"` and channel `"general"`.

## Investigation

- Each test calls `Node::boot()`, which opens its OWN Zenoh session — so the hypothesis "a
  shared session" was ruled out.
- But cargo runs a test binary's tests on multiple threads in ONE process. Embedded Zenoh
  peers in the same process **auto-discover each other** (peer-to-peer scouting) and share a
  single keyspace. Two sessions publishing/subscribing `ws/acme/chan/general/**` therefore see
  each other's traffic.
- Key observation: the workspace prefix `ws/{id}/` is the *only* thing that scopes a key. With
  every test using `ws = "acme"`, the keys collided across tests by construction.

## Root cause

Not a product bug — it is the multi-node design working: two peers that share a workspace DO
share that workspace's bus (that is how a second node will receive messages at S3). The bug was
in the **test**: reusing the same workspace id across concurrently-running tests made their
buses legitimately one bus.

## Fix

Fix at the test layer: a `unique_ws(tag)` fixture gives each test its own workspace id, so
concurrent in-process peers cannot collide — which is *also* the correct semantic, since the
workspace is the isolation wall (§7). The cross-workspace isolation test still uses two
explicitly different ids and asserts no leak. No product code changed; the wall already worked.

- `rust/crates/host/tests/messaging_test.rs`, `messaging_isolation_test.rs`,
  `messaging_deny_test.rs` — each test derives a unique workspace id.

## Verification

`cargo test -p lb-host` (all messaging tests, full concurrency) — green, repeatably.

## Prevention

Standing rule recorded in the bus scope's testing plan: **bus tests must use a unique
workspace id per test** (in-process Zenoh peers share a workspace's keyspace by design). The
isolation test encodes the converse — distinct ids never cross — so a regression in the wall
fails loudly.
