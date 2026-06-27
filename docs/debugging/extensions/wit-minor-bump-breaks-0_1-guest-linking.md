# A WIT `@0.2.0` minor bump breaks every `@0.1.0` guest at instantiation

- **Area:** extensions (SDK/WIT boundary, host-callback scope)
- **Status:** resolved
- **First seen:** 2026-06-27, building the host-callback ABI slice (`sessions/extensions/host-callback-session.md`).

## Symptom

After bumping the WIT package from `@0.1.0` to `@0.2.0` (adding the `host.call-tool` import), every
existing `@0.1.0` guest (`hello`, `github-bridge`) failed to load — the workspace test suite went red at
`github_bridge_normalize_test` / `spine`-adjacent paths with:

```
failed to instantiate component: component imports instance `lazybones:ext/host@0.1.0`,
  but a matching implementation was not found in the linker
```

and, after a first partial fix, the dual:

```
failed to instantiate component: no exported instance named `lazybones:ext/tool@0.2.0`
```

The `lb_sdk::world_major_matches` check (major-only, treats `0.1`/`0.2` as compatible) passed — so the
loader *accepted* the guest — but wasmtime then refused to instantiate it.

## Root cause

**wasmtime's component linker treats a `0.x` MINOR difference as semver-INCOMPATIBLE.** Per Cargo
semver, `0.1.0` and `0.2.0` are a breaking change (only `0.x.y`→`0.x.z` is compatible). So:

- a `@0.1.0` guest's **import** `lazybones:ext/host@0.1.0` is NOT satisfied by the host's `@0.2.0`
  `host` instance in the linker; and
- the host's `@0.2.0` bindings look for the guest's **export** `lazybones:ext/tool@0.2.0`, but a
  `@0.1.0` guest exports `tool@0.1.0`.

Bumping the WIT *package* version bumped BOTH interfaces (`host` AND the unchanged `tool`) together, so
even though `tool` was byte-identical, its version moved. The host's major-only compat check is more
lenient than wasmtime's link-time semver matching — that mismatch is the bug.

## Fix

Make BOTH ABI generations coexist in the runtime (`crates/runtime/`):

1. **Frozen 0.1.0 snapshot** — `sdk/wit-compat-0_1/world.wit`, a verbatim copy of the original 0.1.0
   world (`host.log` + `tool.call`). It must never change.
2. **Link both `host` versions** — `compat_v0_1::add_to_linker` registers the `@0.1.0` `host` interface
   (`log`) alongside the `@0.2.0` one (`engine.rs`). The linker map holds distinct versioned names, so a
   0.1.0 guest's import resolves to the 0.1.0 `host` and a 0.2.0 guest's to the 0.2.0 `host`.
3. **Fall back on the export** — `Engine::load` tries `Extension::instantiate_async` (0.2.0) first; on
   the "no exported instance `tool@0.2.0`" error it instantiates with the 0.1.0 bindings. `Instance`
   holds a `Bindings { V2 | V1 }` enum and dispatches `tool.call` through whichever generation loaded.
   The `tool.call` export is byte-identical across versions, so calling it through either is the same
   wasm call.

World **major** is still `0`, so the `lb_sdk` loader check is unchanged — the minor bump stays backward
safe, now in practice as well as on paper.

## Regression test

- `crates/host/tests/proof_panel_test.rs::hello_v0_1_guest_still_loads_alongside_a_v0_2_callback_guest`
  — loads the prebuilt `@0.1.0` `hello` guest AND the `@0.2.0` `proof-panel` callback guest on ONE node
  and calls each (echo round-trips; derive runs). Fails-before (the instantiate errors above),
  passes-after.
- `crates/host/tests/github_bridge_normalize_test.rs` + `spine_test.rs` (existing) are now also de-facto
  ABI-compat guards — both exercise `@0.1.0` guests on the `@0.2.0` host.
- `sdk/src/lib.rs::matches_same_major` asserts `@0.1.0`, `@0.2.0`, `@0.9.4` all match major 0.

## Lesson

A WIT minor bump is NOT free for existing guests just because the loader's major-check passes — the
component linker enforces semver at link time, and under `0.x` a minor IS breaking there. If you must
add to a versioned interface, either (a) keep the old version linkable (this fix) or (b) only ever add
in a way the toolchain resolves. Adding an import to a *new* interface version means the host must serve
both versions until every guest is rebuilt. Cross-link: `sessions/extensions/host-callback-session.md`,
the scope `scope/extensions/host-callback-scope.md`, and
`debugging/extensions/loaded-extension-instance-is-node-global.md` (why the carried identity is per-call).
