# Crate layout scope

Status: scope. **S0 decision doc** — this is one of the README §13 "forever" decisions
(the SDK/WIT boundary shape) plus the concrete Cargo workspace. Promotes to
`public/crate-layout/` once the workspace builds green.

> Read with: `../../README.md` §9 (the crate map), `../../FILE-LAYOUT.md` (one verb per
> file), `../../STAGES.md` S0 (the exit gate).

---

## Goal

Stand up **one Cargo workspace** under `rust/` with the crate split from README §9, so that
`cargo build` is green on an empty workspace and the SDK/WIT boundary is fixed before any
extension depends on it.

## Non-goals (S0)

- No real implementations inside the crates — S0 is the skeleton. The spine fills them in S1.
- No frontend workspace (separate from the Rust workspace; lands at S2).
- No role-only crates with bodies yet (`gateway`, `ai-gateway`, `registry-host`,
  `bootstrap-ui`) — they exist as empty placeholder crates so the role boundary is visible,
  but carry no logic until their stage.

---

## The workspace

One virtual workspace manifest at `rust/Cargo.toml` (`[workspace]`, no root package). Members
live under `rust/crates/*`, the binary under `rust/node`, the SDK under `rust/sdk`, and the
trivial S1 example extension under `rust/extensions/hello`.

```
rust/
  Cargo.toml                  ← [workspace] virtual manifest, shared [workspace.dependencies]
  rust-toolchain.toml         ← pin the toolchain (reproducible CI)
  crates/
    host/                     ← the kernel: wires store+bus+caps+mcp+runtime together
    store/                    ← SurrealDB wrapper (embedded, one datastore)
    bus/                      ← Zenoh wrapper (peer/router by config)
    runtime/                  ← wasmtime + native sidecar supervisor
    mcp/                      ← rmcp server + tool routing
    auth/                     ← identity, token mint/verify, principal
    caps/                     ← capability grammar, grant store, the check
    tags/                     ← key:value tagging (stub in S0)
    inbox/                    ← generic inbox/outbox (stub in S0)
    jobs/                     ← SurrealDB-native job queue (stub in S0)
    secrets/                  ← envelope-encrypted secrets (stub in S0)
    sync/                     ← edge↔cloud authority sync (stub in S0)
    ext-loader/               ← manifest parse + load/supervise extensions
  sdk/                        ← the STABLE boundary: WIT + Rust guest bindings
    wit/                      ← *.wit — the versioned contract
  node/                       ← the `node` binary: reads config, selects roles
  extensions/
    hello/                    ← the trivial S1 WASM extension (guest crate)
  role/                       ← role-only crates (placeholders in S0)
    gateway/                  ← SSE/HTTP (cloud) — empty until S3
    ai-gateway/               ← model/provider gateway (cloud) — empty until S5
    registry-host/            ← extension registry host (cloud) — empty until S7
    bootstrap-ui/             ← first-run super-admin mint (cloud) — empty until S3
```

Why `role/` is a sibling, not inside `crates/`: README §3.1 says role differences live in
**two thin layers only** (entry/UI + role/deployment). Putting them in their own directory
makes the rule physical — a reviewer can see at a glance that nothing in `crates/` is
role-aware. No `if cloud {…}` in `crates/*`; the `node` binary picks which role crates to
wire by config.

## Crate dependency direction (acyclic)

```
node ─┬─> host ─┬─> store
      │         ├─> bus
      │         ├─> caps ──> auth
      │         ├─> mcp ───> caps
      │         ├─> runtime ─> ext-loader ─> sdk(host side)
      │         └─> (tags, inbox, jobs, secrets, sync)
      └─> role/* (gateway, …) ──> host
```

- **`auth` is a leaf** (principal + token shape; no store dependency in S0 — verify is pure
  given a key). `caps` depends on `auth` for the principal, on `store` for the grant records.
- **`mcp` depends on `caps`** because every tool call is capability-checked before dispatch.
- **`sdk` is depended on from two sides**: the host (`ext-loader`/`runtime` use the host view
  of the WIT) and the guest extensions (`extensions/hello`). See SDK/WIT decision below.
- No crate depends on `node` or on a `role/*` crate. The graph is a DAG; CI can assert it.

---

## DECISION (forever): the SDK / WIT boundary shape

This is README §13's "SDK/WIT boundary" decision. **The stable plugin ABI is the WASI 0.2
Component Model boundary defined in WIT** — never a Rust `dyn Trait` or `cdylib` (Rust has no
stable ABI; §9). Concretely:

- **`sdk/wit/` holds the versioned contract.** A `world lazybones:ext/extension@0.1.0` with:
  - imports (host → guest provides nothing the host calls beyond `handle`): the **host
    functions** a guest may call — `caps.check`, `store.query`, `bus.publish`, `secrets.get`,
    `log` — each capability-gated host-side.
  - exports (guest → host): `init(manifest-config)` and `call-tool(name, json-input) ->
    result<json-output, tool-error>`. An extension *is* a bundle of MCP tools (README §6.5,
    §7), so the export surface is deliberately tiny: one tool-dispatch entry point.
- **Versioning is semver on the WIT package** (`@0.1.0`). Breaking the world = a new
  major; the loader refuses a component whose imported world major doesn't match the host.
  This is the §11.2 "forever commitment" — we make the surface as small as possible so it is
  cheap to keep stable.
- **The Rust SDK crate (`sdk/`) is a thin guest-side convenience** over `wit-bindgen`: it
  re-exports the generated bindings and offers a `#[tool]`-style ergonomic layer later. In S0
  it is just the WIT + a generated-bindings stub so the boundary compiles.
- **The host side** (`runtime` + `ext-loader`) consumes the same WIT via `wasmtime`'s
  component bindgen. One `.wit`, two generated sides — the contract can't drift.

**Rejected:** a native Rust trait object plugin API (no stable ABI, can't sandbox, breaks
hot-reload safety). **Rejected:** gRPC/JSON-RPC over a socket as the *primary* tier (that's
the Tier-2 native escape hatch, README §6.3 — not the default). WASM Component Model is the
default; native sidecars are the exception.

---

## How it fits the core

- **Symmetric nodes:** the role boundary is the `crates/` vs `role/` directory split; the
  `node` binary is the one place roles are selected, by config. No core crate is role-aware.
- **One datastore / state vs motion:** `store` (SurrealDB) and `bus` (Zenoh) are separate
  crates and never depend on each other.
- **Capability-first:** `mcp` depends on `caps`; there is no dispatch path that skips it.
- **One responsibility per file:** each crate is itself a folder-of-verbs (FILE-LAYOUT); no
  `utils`/`common` crate exists.

## Testing plan

- **S0:** `cargo build --workspace` green; a `dep-graph` check (or a doc note for now) that no
  `crates/*` crate names a `role/*` crate or `node`. CI runs the FILE-LAYOUT size check.
- **S1 onward:** each crate grows its own `tests/` per `testing-scope.md`; the mandatory
  capability-deny and workspace-isolation tests live where the spine is wired (`host`/`mcp`).

## Open questions

- Does `sdk/` publish to crates.io eventually, or stay path-only until the registry (S7)?
  → Defer; path-only through S6.
- Workspace-level `[workspace.lints]` to enforce a max-warnings policy in CI? → Add when the
  first real lints bite; not S0.
- Do `tags`/`inbox`/`jobs`/`secrets`/`sync` need to exist as crates in S0 if empty, or join
  at their stage? → **Exist now as stubs** so the dependency graph and the §9 map are real
  from day one; cheaper than re-shuffling later.
