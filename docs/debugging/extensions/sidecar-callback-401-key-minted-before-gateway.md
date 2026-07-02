# A native sidecar's callbacks 401 — its `LB_EXT_TOKEN` was minted before the gateway installed its key

**Area:** extensions (native-tier callback transport) · **Status:** resolved · **Date:** 2026-07-02

## Symptom

Driving the `control-engine` federated page against a real `make dev`-style node, every call
that used the sidecar's **host callback** failed:

```
/native/call control-engine.appliance.list
→ supervisor: child returned an error: host callback failed: host returned HTTP 401: invalid or expired credential
```

Graph reads that DON'T call back (they reach the engine directly) instead failed later at the
engine hop; but anything touching `store.*` via the callback (`appliance.add`/`list`/`remove`,
the boot appliance-seed) 401'd. The extension's own grant was correct (the install log showed all
the expected caps), and the caller's token was valid — so "invalid credential" pointed at neither.

## Root cause — boot ordering, one signing identity per node

A native sidecar's `LB_EXT_TOKEN` is minted by `install_native` with **`node.key()`** so the
gateway can verify it on the callback (native-callback-transport scope: ONE signing identity per
node). `Gateway::new_live` is what installs that shared key onto the node (`node.install_key`).

In `rust/node/src/main.rs` the native roles were mounted **before** the gateway was created:

```
federation::mount(node)        // install_native → mints LB_EXT_TOKEN with node's THROWAWAY boot key
control_engine::mount(node)    //   (Node::new seeded a random key at line ~90)
...
Gateway::new_live(node, key)   // NOW overwrites node.key() with the gateway's key
serve(gw)
```

So each sidecar's token was signed with the node's throwaway boot key, then the gateway replaced
the node key with its own. On the callback the gateway verified with its key → signature mismatch →
`401 invalid or expired credential`. `federation`'s boot seed had been silently hitting the same
wall ("seed datasource skipped: denied") — `control-engine` is just the first sidecar whose PAGE
exercises the callback in normal use, so it surfaced the latent bug.

## Fix

Mount the native sidecar roles **after** `Gateway::new_live` (which installs the shared key), so
`install_native` mints each child's token with the final `node.key()` the gateway verifies with:

```rust
let gw = lb_role_gateway::Gateway::new_live(node.clone(), SigningKey::generate());
federation::mount(node.clone()).await;      // now minted with the shared key
control_engine::mount(node.clone()).await;
serve(gw, addr).await?;
```

(The `else` branch — no `LB_GATEWAY_ADDR`, edge/solo posture — still mounts them for a headless
node; their callbacks degrade cleanly with no gateway.)

## Proof

After the fix, against a real gateway: `/native/call control-engine.appliance.add` →
`{"id":"local"}` **200**, `appliance.list` → the seeded appliance **200** (was 401). The remaining
`tree` failure was only the ce-studio engine being offline — every LB-side hop passed.

## Lesson

A native sidecar's callback token must be minted with the SAME key the gateway verifies with. That
key is installed by `Gateway::new_live`, so **mount native roles after the gateway is built**, never
before. Boot ordering is load-bearing for the callback transport; a "401 invalid credential" on a
sidecar callback with correct caps is a key-mismatch, not a caps problem.

Related: [ce-tree-fails-without-gateway-real-engine-tier.md](ce-tree-fails-without-gateway-real-engine-tier.md)
(the sibling "no callback address" failure when `LB_GATEWAY_URL` is unset).
