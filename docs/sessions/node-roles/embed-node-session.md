# Embed-node session — `lb-node` lib target + `boot_full(BootConfig)` seam

Scope: [`docs/scope/node-roles/embed-node-scope.md`](../../scope/node-roles/embed-node-scope.md).
Phase 2a of the embed-node coding session. Status: **built + tested green** (working tree, not
git-committed).

## The ask

Give the `node` package a **library** target exposing a supported embed API — `BootConfig` +
`boot_full(cfg) -> RunningNode` performing the boot ritual ONCE — and refactor `main.rs` onto it so the
binary shrinks to `boot_full(BootConfig::from_env()).await` + serve. A third-party embedder
(`NubeIO/rubix-ai`) will git-dep `lb-node` and call `boot_full`.

## What changed

### Package rename (open question RESOLVED)
- `rust/node/Cargo.toml`: `[package] name = "node"` → **`lb-node`**. Binary stays `node`
  (`[[bin]] name = "node"`), lib is **`lb_node`** (`[lib] name = "lb_node" path = "src/lib.rs"`).
  `version = "0.1.9"` kept (the core-skills seeder keys off `CARGO_PKG_VERSION`).
- The DIRECTORY stays `rust/node/`; the workspace member `"node"` (a path) is unchanged.
- Every `cargo …-p node` was repointed to `-p lb-node` (`-p` is a *package* selector):
  - `Makefile`: `NODE_BIN := node` → `lb-node` (used in 3 `cargo run -p $(NODE_BIN)` recipes).
  - `deploy/common/Dockerfile`: `cargo build … -p node` → `-p lb-node` (the compiled binary path
    `target/release/node` is unchanged — bin name is still `node`).
  - The `external-agent` doc comment in `Cargo.toml`.
  - Grepped the whole tree (`-p node`, `--package node`, `--bin node`): no other references.

### The lib decomposition (folder of verbs, one responsibility per file)
`rust/node/src/`:
- `lib.rs` (32) — barrel: `pub mod …; pub use builder::{boot_full, RunningNode}; pub use
  config::{AgentModelConfig, BootConfig, GatewayMode}`.
- `config.rs` (222) — `BootConfig` (`#[non_exhaustive]` + `Default`, all-`pub`), `GatewayMode`
  (`Off`/`Addr`), `AgentModelConfig`, and `BootConfig::from_env()` — the **only** place `LB_*` boot
  vars are read. `gateway_signing_key` moved here verbatim from `main.rs`.
- `builder.rs` (144) — `boot_full(cfg)` (the one ritual) + `RunningNode` + `RunningNode::serve()` +
  `open_store(cfg)` (the store-path→`Store` selection, sourced from the struct, not env).
- `seeds.rs` (81) — the seed sequence (identity/core-skills/agent-defs/personas/migration/default
  grants), all idempotent + best-effort.
- `reactors.rs` (43) — the flow/agent/approval/insight-digest spawns + the one-shot insight-ts heal.
- `seed_identity.rs` (30) — `seed_dev_identity`, moved verbatim from `main.rs`.
- `hello_demo.rs` (61) — the S1 `hello` load + `hello.echo` call, moved from `main.rs`, now gated.
- `agent.rs`/`federation.rs`/`control_engine.rs`/`external_agent.rs` — the thin role mounts, kept.
- `main.rs` (17) — **thin**: `boot_full(BootConfig::from_env()).await?; running.serve().await`.

### `BootConfig` fields (all `pub`, from today's env)
`store_path` (LB_STORE_PATH) · `signing_key` (LB_SIGNING_KEY) · `workspace` (LB_WORKSPACE, def `acme`)
· `seed_user` (LB_SEED_USER, def `user:ada`; `None` skips) · `gateway: GatewayMode`
(LB_GATEWAY_ADDR presence) · `reactors: bool` · `hello_demo: bool` · `default_core_skills`
(LB_DEFAULT_CORE_SKILLS) · `telemetry: SinkConfig` (LB_TELEMETRY_SINK) · `agent_model: AgentModelConfig`
(LB_AGENT_MODEL_*) · `agent_caps: Option<Vec<String>>` (LB_AGENT_CAPS).

### `RunningNode` hands back (open question — DECIDED)
`{ node: Arc<Node>, gateway: Option<(Gateway, SocketAddr)>, agent_server: Option<AgentServer> }`.
The embedder calls host verbs in-process on `node`; `serve()` blocks serving the gateway (a no-op
returning `Ok(())` when gateway is `Off`) and holds `agent_server` alive for the duration. The fields
are `pub` so a `shutdown()` (reactor cancellation + sidecar shutdown token) can be added additively
later — teardown is deferred (see open questions).

## Config vs drift decisions (per-divergence archaeology)

- **`hello_demo`**: today's `main.rs` loads `hello` + calls `hello.echo` UNCONDITIONALLY. That is
  `hello_demo: true` in `from_env()` (exact parity) but `false` in `Default` — an embedder does not want
  a demo extension. This is **config**, not drift (both behaviours are intentional postures).
- **`seed_user: None`**: `from_env()` always sets `Some("user:ada")` (parity). `None` (skip the dev
  seed) is a new embedder posture — config.
- **Store**: `main.rs` relied on `Node::boot()` reading `LB_STORE_PATH` inside `open_store()`. The lib
  MUST NOT read env below the seam, so `boot_full` uses `Node::boot_with_store(open_store(&cfg))` and
  does the same config-not-role path/`mem://` selection from the STRUCT. Behaviour identical; the env
  read moved up to `from_env()`. Not drift — the same selection, relocated to the binary boundary.
- **Load-bearing ordering preserved EXACTLY**: native roles (federation, control-engine) + agent mount
  AFTER `Gateway::new_live` installs the signing key onto the node (else sidecar callbacks 401). With
  gateway `Off`, the node's boot key stands and the roles mount headless — same as today's `else` arm.

### `#[non_exhaustive]` construction (design note)
The scope's example shows `BootConfig { store_path: Some(..), ..Default::default() }`, but
`#[non_exhaustive]` deliberately **forbids** a cross-crate struct literal — that forbiddance is exactly
what makes "additive fields never break an embedder" hold. The supported construction path is therefore
`let mut c = BootConfig::default(); c.store_path = Some(..);` (all fields `pub`). Documented in the
`config.rs` module doc and used in the test. (A fluent `NodeBuilder` was considered; `default()`-mutate
is lighter and sufficient — deferred unless an embedder asks.)

## Deferred (explicit, not silent): role-mount de-env'ing

The CORE ritual (store / key / workspace / seeds / reactors / gateway-addr / telemetry / agent model +
caps) is **fully struct-config** — no env below the seam. The three role mounts
(`federation::mount`, `control_engine::mount`) still read their OWN `LB_FEDERATION_*` /
`LB_CONTROL_ENGINE_*` env internally, and `hello_demo`/`federation`/`control_engine` still resolve
sidecar dirs via `CARGO_MANIFEST_DIR`/`LB_*_DIR`. De-env'ing those (passing a `FederationConfig` /
`ControlEngineConfig` through `BootConfig`) is a bounded follow-up — they read many vars and use
`include_str!` sidecar-manifest wiring, so folding them in one pass risked the load-bearing deliverable.
`agent::mount` WAS de-env'd (takes `&AgentModelConfig` + `Option<caps>`; the api-key env-NAME lookup
stays, since only the NAME is config and the VALUE is legitimately a process-env secret read at the
binary boundary). A `GatewayMode::Listener` variant (hand the gateway back for the embedder's own
`serve_listener`) is likewise deferred — `serve_listener` exists but is not yet threaded through.

## Tests (real infra, `mem://` store, no mocks)

`rust/node/tests/embed_test.rs` (integration test against the lib, boots via `boot_full`):
- `embedded_node_denies_a_caller_without_the_cap` — MANDATORY capability-deny: no-cap caller gets
  `ToolError::Denied` on `ingest.write` through the embedded node; with the cap it succeeds.
- `embedded_node_isolates_workspaces` — MANDATORY workspace-isolation: ws-B reading its own namespace
  sees nothing of ws-A's; a ws-B token asking for ws-A is `Denied` at gate 1.
- `from_env_defaults_match_the_binary` — parity guard on `BootConfig::from_env()` defaults.

Plus the existing `agent::tests` (adapter selection) still pass under the lib target.

### Green output

```
$ cd rust && cargo build --workspace          # EXIT=0
$ cd rust && cargo build -p lb-node --features external-agent   # EXIT=0
$ cd rust && cargo test -p lb-node
running 2 tests
test agent::tests::an_unknown_provider_has_no_adapter ... ok
test agent::tests::known_providers_build_a_configured_model ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 3 tests
test from_env_defaults_match_the_binary ... ok
test embedded_node_isolates_workspaces ... ok
test embedded_node_denies_a_caller_without_the_cap ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests lb_node ... ok
```

`cargo fmt` run; all `node/src/*.rs` files under the 400-line hard limit (largest: agent.rs 264).

## Follow-ups

- De-env the `federation` / `control_engine` role mounts (config structs on `BootConfig`).
- `GatewayMode::Listener` + a real `RunningNode::shutdown()` (reactor cancel + sidecar shutdown token).
- Refactor the OTHER two embedders (`ui/src-tauri/src/full.rs`, `role/gateway/.../test_gateway.rs`)
  onto `boot_full` — the scope's "refactor-as-proof" (Phase 2b). Not done this pass.
