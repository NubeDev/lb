# Crate layout (shipped — S1)

The Rust workspace as it exists now. Scope/decisions: `../../scope/crate-layout/crate-layout-scope.md`.
Session: `../../sessions/core/s0-s1-spine-session.md`.

## The workspace

One virtual Cargo workspace at `rust/Cargo.toml`. Members:

```
rust/
  crates/        host store bus runtime mcp auth caps  (S1: real)
                 tags inbox jobs secrets sync          (S1: stubs, fill at their stage)
                 ext-loader                            (S1: real)
  sdk/           the stable WIT boundary (sdk/wit/world.wit) + version constants
  node/          the `node` binary — boots a solo node, selects roles by config
  extensions/
    hello/       the wasm32-wasip2 component (EXCLUDED from the workspace — different target)
  role/          gateway ai-gateway registry-host bootstrap-ui  (placeholders until S3/S5/S7)
```

`role/` is a sibling of `crates/`, not inside it — so "no core crate is role-aware"
(README §3.1) is physically visible. The `node` binary is the one place roles are selected.

## Dependency direction (acyclic)

`node → host → {store, bus, caps→auth, mcp→caps, runtime→ext-loader→sdk, …}`. No crate
depends on `node` or a `role/*` crate.

## The stable boundary

`sdk/wit/world.wit` defines `world extension` (`lazybones:ext/extension@0.1.0`): exports
`tool.call(name, json) -> result<json, tool-error>`, imports `host.log`. The host
(`runtime`) and guests (`extensions/hello`) generate bindings from this *one* file, so the
ABI cannot drift. The loader refuses a component whose world major ≠ the host's.

## Build & test

```
# the wasm guest (separate target; CI builds it first)
(cd rust/extensions/hello && cargo build --target wasm32-wasip2 --release)
# the host workspace
(cd rust && cargo build --workspace && cargo test --workspace)
```

CI (`.github/workflows/ci.yml`): FILE-LAYOUT size check → build wasm guest → build/test → fmt.
