# Session — native (Tier-2) extensions never respawn at boot (issue #64)

**Date:** 2026-07-14 · **Area:** extensions / lifecycle-management · **Issue:** [#64](https://github.com/NubeDev/lb/issues/64)

## The ask

A published `tier="native"` extension does not come back after a node restart:
`Install.enabled=true`, `GET /extensions` says `running=false health=stopped`, and the
boot log says **nothing at all**. Wasm extensions are unaffected. Fix it generically,
mirror the wasm half, add the boot log line, and add the integration test the issue names.

## Verifying the diagnosis before building on it

The issue's diagnosis was checked against the code rather than taken on trust (the reporter
had been wrong twice before reading it properly). It holds, exactly:

- `ext/reconcile.rs` computes the plan correctly and **does** handle `Tier::Native`.
- `ext/boot_load.rs::load_enabled` consumes the plan and skips every native action
  (`if action.tier != "wasm" { continue }`), deferring to "the node Launcher's job".
- **No node implements that path.** `grep` for non-test consumers of the plan finds exactly
  one: `rust/node/src/builder.rs:85`, which calls `load_enabled` — the wasm-only verb.
  Nothing anywhere acts on a native `ReconcileAction`. The plan's native actions are
  computed and dropped on the floor.

So: intent durable, plan correct, nothing executes it. Confirmed independently.

## What shipped

Mirrors the wasm half rather than inventing a second mechanism.

| File | Role |
|---|---|
| `crates/host/src/ext/boot_spawn.rs` | **new** — `spawn_enabled`, the native peer of `load_enabled`. Generic over `L: Launcher` (as `install_native` already is), so the node passes `OsLauncher` and a test passes its own. |
| `crates/host/src/ext/install_dir.rs` | **new** — `native_install_dir` + `write_executable`, extracted from `publish.rs` so publish and boot share ONE copy of the `(ws, ext)` → dir rule. |
| `crates/host/src/ext/publish.rs` | now imports those two instead of holding private copies. |
| `crates/host/src/ext/mod.rs`, `lib.rs` | wire + export `spawn_enabled` / `SpawnedExt`. |
| `node/src/builder.rs` | **the actual fix**: the node now calls `spawn_enabled` at boot, and logs every native extension — including the ones that did not come back. |
| `crates/host/tests/ext_boot_spawn_test.rs` | **new** — 4 tests over a real supervised OS child. |

**No new persistence, no new trust.** `resolve` → `read_cached` already yields
`manifest_toml` + verified bytes — exactly what `install_native` takes. The install dir is
re-derived from `(ws, ext)`, not stored. The bytes were verified before they were cached.

### Three decisions worth recording

**1. Where the call sits — after the gateway block, not beside `load_enabled`.**
The two boot halves are deliberately in different places. `install_native` mints each child's
`LB_EXT_TOKEN` with `node.key()`, and the gateway verifies those callback tokens with its own
key — which it installs onto the node in the gateway block. Respawning up beside `load_enabled`
(the "obvious" symmetric spot) would mint every sidecar's token with the pre-gateway key and
401 every callback. This is the same load-bearing ordering the `federation`/`control-engine`
role mounts already document; the native boot half obeys it for the same reason. Wasm has no
such constraint (no process, no token), so it stays early.

**2. The boot caller — `node:boot`, minted in-process.**
`install_native` gates on `mcp:native.install:call` because a *caller* asking to spawn a
process must hold that grant. Boot is not a caller: nobody is asking, and the wasm peer
(`load_extension`) takes no principal at all for exactly that reason. Rather than widen the
gate, or thread a caller into a boot path (which would invite passing an *untrusted* one),
boot names itself — a `node:boot` sub holding EXACTLY one cap, scoped to the one workspace
being reconciled, never signed, never persisted, unreachable from any request path. It cannot
widen an install either: the grant handed to `install_native` comes from the **durable
`Install.granted`**, never the manifest's `requested`, so a restart reproduces exactly the
privilege an admin approved and cannot re-approve what they narrowed. Missing record ⇒ empty
set (fail-closed), not `requested` (fail-open).

**3. The open question the issue raised: hard-fail boot, or log-and-continue?**
**Log-and-continue**, deliberately. Hard-failing turns one broken extension into a node that
will not start — and the recovery path for a bad extension (publish a fix, disable it) runs
*through the node it just killed, over the gateway that never came up*. That trades a degraded
node for an unbootable one and can strand an unattended box. It also matches every neighbouring
boot step (`load_enabled`, seeds, role mounts).

The counter-argument ("silently degraded is what made this expensive") is right about the
symptom but wrong about the cause: what cost hours was the **silence**, not the continuing. So
the silence is what got fixed — an enabled native extension that is not running now says so on
stderr, every boot, by name and reason:

```
boot: native extension <id>@<ver> not started by boot bring-up (no-cached-artifact) — it is installed and enabled; if nothing else starts it, it is not running
```

The wording is deliberately narrow. It states what the verb **knows** — durable intent said run, boot did not start it, here is the reason — and stops short of "it is not running", because an embedder may mount its own sidecars directly after `boot_full` returns. Over-claiming would send an operator hunting a healthy extension, which is the opposite of the point. (Caught in live verification, where exactly that happened.)

An operator who wants "no degraded boots" can build that on this output. An operator whose node
is one broken extension away from unreachable cannot un-build a panic.

## Tests — and proof they can fail

`crates/host/tests/ext_boot_spawn_test.rs`, all against the **real** `echo-sidecar` OS child
over a **real on-disk store that outlives the node** (rule 9 — a real process is the one true
external):

1. `a_published_native_extension_respawns_on_boot_and_answers` — the headline. Publish → drop
   the node (the `SidecarMap` dies with it, exactly as a process restart kills every child) →
   re-boot on the same store → `spawn_enabled` → the child is live **and answers a tool call**
   with its scoped identity intact. No republish.
2. `a_disabled_native_install_stays_down_across_a_restart` — durable intent outranks respawn.
3. `an_enabled_install_with_no_cached_artifact_is_reported_not_silent` — the visibility half.
4. `a_second_bring_up_does_not_double_spawn` — idempotent against the live runtime.

**Revert-checked** (the rule that matters here): with the native branch put back the way it was
(`if action.tier != "wasm" { continue }`), **all 4 fail** — with the bug's own signature, an
empty boot log: `no boot-log row for echo-sidecar, got []`. A test that cannot fail is worse
than no test, and this issue exists *because* a fully-unimplemented branch sat behind a green
suite.

## Gates

- `cargo fmt --check` clean; no new clippy warnings from these files.
- `cargo test -p lb-host --test ext_boot_spawn_test` → 4/4.
- Neighbours green, no regressions: `ext_lifecycle_test` 4/4, `ext_publish_test` 5/5,
  `native_test` 5/5, `install_record_test` 2/2, `lb-ingest` 17/17.

## Live verification

The bar the issue sets is the product, not the suite — this bug was invisible to a green suite
and visible in seconds against a running node. Verified end to end on a real node with a real
native sidecar published over `POST /extensions`:

1. publish a native extension → `running=true health=ok`;
2. **restart the node only**, store intact, no republish;
3. `GET /extensions` → `running=true health=ok`, and the boot log says
   `boot-spawned native extension: <id>@<ver>`.

Step 3 is what fails on `master` today (`running=false health=stopped`, silent boot log). The
downstream product check that depends on that sidecar polling also passes after the restart.

The live run also earned the log-wording fix above: the boot line reported an extension the
**embedder** mounts itself (after `boot_full` returns) as "not running", which was true at
boot-log time but misleading by the time an operator read it.

## Follow-ups (not in scope here)

- **No start/restart endpoint.** `/extensions/<ext>/enable` returns 204 but does not spawn a
  stopped sidecar; republishing is still the only interactive recovery. Boot now covers the
  restart path, but an operator who wants to start a stopped native ext *without* a restart
  still has no verb. Worth its own issue.
- `ext.publish` on a native extension requires **both** `mcp:ext.publish:call` and
  `mcp:native.install:call` (publish performs the spawn). Correct — the native tier is not
  special-cased out of its own gate — but undocumented, and it surfaced here as an opaque
  `Native("denied")` from a *publish* call, which is a confusing error to debug.
