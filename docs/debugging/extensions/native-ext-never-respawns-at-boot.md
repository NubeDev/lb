# native (Tier-2) extensions never respawn at boot — the plan was computed and dropped

**Date:** 2026-07-14 · **Area:** extensions / lifecycle-management · **Status:** fixed
**Issue:** [#64](https://github.com/NubeDev/lb/issues/64)

## Symptom

A published `tier="native"` extension does not come back after a node restart. Its durable
`Install` record says `enabled: true`, `GET /extensions` reports `running=false
health=stopped`, and **the node logs nothing at all about it at boot** — no spawn attempt,
no warning, no error. Wasm extensions are unaffected.

The silence is the severity: an operator sees a healthy node, a healthy UI, and an extension
the registry lists as installed — while the child process simply does not exist. Every
downstream symptom (a poller that never polls, values ageing forever) looks like a bug in the
extension rather than a boot gap. Republishing was the only recovery
(`/extensions/<ext>/enable` returns 204 but does not spawn).

Found live against a real native sidecar in a downstream product, not by `cargo test`.

## Root cause — the native half of boot bring-up was never wired

The design was complete and documented; only the native branch was unimplemented.

- `ext/reconcile.rs` computes the boot plan correctly and **does** handle `Tier::Native`
  (`is_running` → `start`/`already-running`/`disabled`).
- `ext/boot_load.rs::load_enabled` consumes the plan and skips every native action —
  `if action.tier != "wasm" { continue }` — deferring to "the node Launcher's job". That
  deferral is reasonable in itself: `install_native` needs the `Launcher` + `install_dir` the
  node binary owns, which is why `reconcile` returns a plan rather than spawning.
- **But no node implemented that path.** The only non-test consumer of the plan is
  `node/src/builder.rs`, which calls `load_enabled` — the wasm-only verb. Nothing anywhere
  acted on a native `ReconcileAction`. The plan's native actions were computed and dropped.

Intent durable, plan correct, nothing executing it. The code promised otherwise in three
places (`Install.enabled` "the boot reconciler honors enabled ∧ started"; `native/mod.rs`
"a runtime-only cache the records can rebuild"; `boot_load.rs` "this is what makes a published
extension survive a restart") — all true for wasm only.

## Fix

Mirror the wasm half; do not invent a second mechanism.

- **`ext/boot_spawn.rs`** (new) — `spawn_enabled`, the native peer of `load_enabled`, generic
  over `L: Launcher` (as `install_native` already is). For every native action marked `start`:
  `resolve` → `read_cached` → land the binary → `install_native`. **No new persistence, no new
  trust** — the cache already held `manifest_toml` + verified bytes, which is exactly the pair
  `install_native` takes, and the bytes were verified before they were cached.
- **`ext/install_dir.rs`** (new) — `native_install_dir` + `write_executable` extracted from
  `publish.rs` so publish and boot share ONE copy of the `(ws, ext)` → dir rule. Boot
  re-derives publish's path instead of persisting it; two copies of that rule would mean a
  published extension that silently fails to respawn.
- **`node/src/builder.rs`** — the node now calls `spawn_enabled` at boot **and logs every
  native extension**, including the ones that did not come back.

### Two things that are easy to get wrong here

**Placement is load-bearing.** `spawn_enabled` is called *after* the gateway block, NOT beside
`load_enabled`. `install_native` mints each child's `LB_EXT_TOKEN` with `node.key()`, and the
gateway verifies those callback tokens with its own key, which it installs onto the node in the
gateway block. Respawning at the "obvious" symmetric spot (next to the wasm half) would mint
every sidecar's token with the pre-gateway key and **401 every callback** — the same trap the
`federation`/`control-engine` role mounts already document. Wasm has no such constraint (no
process, no token), so the two halves legitimately sit in different places.

**Boot is not a caller.** `install_native` gates on `mcp:native.install:call` because a
human/agent asking to spawn a process must hold that grant; the wasm peer `load_extension`
takes no principal at all, because at boot there is nobody to authenticate. Rather than widen
the gate or thread a caller into a boot path (which would invite passing an *untrusted* one),
boot mints a `node:boot` principal holding EXACTLY that one cap, scoped to the one workspace,
never signed, never persisted. It cannot widen an install: the grant handed to `install_native`
comes from the **durable `Install.granted`**, never the manifest's `requested`, so a restart
reproduces exactly the privilege an admin approved and cannot re-approve what they narrowed
(missing record ⇒ empty set — fail-closed).

## Resolved: hard-fail boot vs log-and-continue

The issue flagged this as open. **Log-and-continue.** Hard-failing turns one broken extension
into a node that will not start — and the recovery path for a bad extension (publish a fix,
disable it) runs *through the node it just killed, over the gateway that never came up*. That
trades a degraded node for an unbootable one and can strand an unattended box.

The counter-argument ("silently degraded is what made this expensive") is right about the
symptom and wrong about the cause: what cost hours was the **silence**, not the continuing. So
the silence is what got fixed — every enabled native extension boot did not bring up now names
itself and its reason on stderr, every boot:

```
boot: native extension <id>@<ver> not started by boot bring-up (no-cached-artifact) — it is installed and enabled; if nothing else starts it, it is not running
```

An operator who wants "no degraded boots" can build that on this output. An operator whose node
is one broken extension away from unreachable cannot un-build a panic.

## Regression tests — and the proof they can fail

`crates/host/tests/ext_boot_spawn_test.rs`, 4 tests over the **real** `echo-sidecar` OS child
on a **real on-disk store that outlives the node** (rule 9):
`a_published_native_extension_respawns_on_boot_and_answers` (publish → drop the node, which
drops the `SidecarMap` exactly as a restart kills every child → re-boot on the same store →
the child is live **and answers a tool call**, no republish),
`a_disabled_native_install_stays_down_across_a_restart`,
`an_enabled_install_with_no_cached_artifact_is_reported_not_silent`,
`a_second_bring_up_does_not_double_spawn`.

**Revert-checked:** with the native branch restored to `if action.tier != "wasm" { continue }`,
**all 4 fail** — with the bug's own signature, the empty boot log:
`no boot-log row for echo-sidecar, got []`.

## Lessons

- **A test that asserts a plan is correct proves nothing about the plan being executed.** The
  pre-existing `boot_reconcile_honors_disable_intent` asserted the reconcile plan was right —
  and it always was. A *fully unimplemented* branch sat behind a green suite for as long as the
  gap existed, because nothing tested the consumer. When a component computes a plan for
  someone else to run, the test that matters is the one that runs it.
- **"Deferred to another layer" is a claim, not a fact — grep for the layer.** The `continue`
  named an owner ("the node Launcher's job") and that owner never existed. A deferral comment
  is a TODO wearing a design decision's clothes unless a caller can be pointed at.
- **Silence is a bug, not a side effect.** The invisibility was half the cost here and got its
  own fix and its own test. A boot that skips work must say what it skipped and why.
- **A diagnostic line must claim only what its layer knows.** The first wording ended "it is
  not running" — and live verification immediately caught it lying: an *embedder* mounts its own
  sidecars directly after `boot_full` returns, so the extension was live seconds later. True at
  boot-log time, misleading by the time a human read it. A log line that over-claims sends an
  operator hunting a healthy component, which is the same wasted afternoon this issue is about,
  just pointed the other way. It now states the intent, the action, and the reason — and stops.

## Follow-up (not fixed here)

**No start/restart endpoint.** `/extensions/<ext>/enable` returns 204 but does not spawn a
stopped sidecar; republishing remains the only interactive recovery. Boot now covers the
restart path, but starting a stopped native ext *without* a node restart still has no verb.
Worth its own issue.
