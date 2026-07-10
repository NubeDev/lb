# Node-roles scope — lb as a Rust library: one boot seam for every embedder

Status: scope (the ask). Promotes to `public/node-roles/node-roles.md` once shipped.

The Rust core can already be used as a library — the Tauri desktop shell (`ui/src-tauri/src/full.rs`)
and the UI test harness (`role/gateway/src/bin/test_gateway.rs`) both boot a full node in-process via
`lb_host::Node` — but there is **no supported seam**: each embedder hand-transcribes the same boot
ritual from `rust/node/src/main.rs` (signing key, dev-identity seed, core-skills seed, agent
definitions, federation supervise, reactors, gateway serve), and configuration is env-vars only. We
want **one library API** — a `BootConfig` struct + a builder that performs the ritual once — that the
node binary, the Tauri shell, the test gateway, and any third-party Rust program all call. **This
stays in the `lb` repo; no new repo.**

## Goals

- **One boot ritual, written once.** The `node` package grows a **lib target** exporting
  `BootConfig` + `NodeBuilder` (or `boot_full(config)`); `main.rs` shrinks to arg/env parsing +
  one call. The three existing embedders are refactored onto it — that refactor is the proof the
  API is sufficient.
- **Struct config with env fallback.** `BootConfig` carries what today is `LB_STORE_PATH`,
  `LB_SIGNING_KEY`, `LB_DIR`, dev-seed identity, federation endpoints/seed, gateway bind addr, and
  the role/reactor toggles. `BootConfig::from_env()` reproduces today's binary behavior exactly;
  embedders pass the struct and **no library code reads env** (the doctrine already stated in
  `ui/src-tauri/src/store.rs`: env is set/read at the *binary* boundary, never in `Node::boot`).
- **Composable subsets.** Builder toggles for the big optional pieces: gateway (serve vs. hand back
  the `Gateway` for the embedder's own listener — `serve_listener` exists), reactors on/off,
  federation supervise on/off, dev-identity seeding on/off. An embedder that wants store + auth +
  MCP only, without HTTP, gets it.
- **A documented, tested embed story:** "add a git dep on `NubeDev/lb`, call `boot_full`" — with a
  real integration test that does exactly that path (in-workspace) and drives the result.

## Non-goals

- **No new repo, and no crates.io publish of `lb-host`.** Embedding is **git-dep against `lb`**.
  Rejected: a separate `lb-embed`/`lb-node` repo — the boot ritual must evolve atomically with
  `lb-host` (every new seed/reactor lands in both), so a second repo recreates exactly the mirror-
  drift problem `ext-out-of-tree-scope.md` works to kill. Publishing `lb-host` to crates.io is a
  far bigger API-freeze commitment than the SDK crates and is explicitly deferred; only the SDK
  surface publishes (ext-out-of-tree).
- **Not the UI.** This is the Rust core only; the shell/UI embed story is Tauri's and unchanged.
- **No config-file format.** `BootConfig` is a Rust struct; how a binary fills it (env today,
  maybe TOML later) is the binary's business, out of scope here.
- **No new roles or role semantics** — `node-roles-scope.md` owns those. This scope moves the
  existing wiring behind one API; it does not change what boots.
- **Not a stability promise on `lb-host`'s full re-export surface.** The stable thing is the boot
  seam (`BootConfig`/`NodeBuilder`); the host verb functions remain semver-honest workspace API.

## Intent / approach

**The lib lives in the `node` package — because §3.1 says so.** The boot ritual selects roles
(gateway, ai-gateway, external-agent, federation supervise), and no core crate under `rust/crates/*`
may be role-aware; the `node` package is precisely the sanctioned thin role-aware layer. So the
package gains `src/lib.rs` (lib name e.g. `lb_node`) holding `BootConfig`, `NodeBuilder`, and the
ritual, with `main.rs` reduced to `boot_full(BootConfig::from_env()).await` plus signal handling.
Rejected alternatives: a new `rust/crates/embed` crate (would be a role-aware core crate — violates
§3.1); leaving it in `lb-host` (same violation — host must not know the gateway/role crates);
a new repo (see non-goals).

**Refactor-as-proof.** The deliverable isn't just the new API — it's `main.rs`,
`ui/src-tauri/src/full.rs`, and `test_gateway.rs` all *calling* it. Each currently carries a
slightly different copy of the ritual; after this scope there is one copy and three thin callers.
Any future seed/reactor added to boot lands in every embedder automatically — today it silently
lands in one and drifts from the others. (`full.rs` keeps its Tauri-specific extras — window state,
store-path resolution — as pre-boot config filling, which is exactly what `BootConfig` is for.)

**Env stays a binary concern.** `from_env()` is the only place `LB_*` boot vars are read, and only
binaries call it. The Tauri shell's current "set env before boot" workaround
(`src-tauri/src/store.rs`) inverts into filling `BootConfig.store_path` directly.

**Feature flags pass through.** The `external-agent` cargo feature keeps its compile-time-optional
contract: the lib exposes the registration hook under the same feature, and an embedder that wants
it builds with the feature on. Off by default, unchanged.

## How it fits the core

- **Symmetric nodes:** strengthened — role selection remains config (`BootConfig`), one ritual, no
  `if cloud`. The builder toggles are the config/role posture made explicit as a type.
- **Tenancy / isolation & capabilities:** unchanged paths — an embedded node runs the same store,
  same caps wall, same gateway gates. The mandatory tests re-run *through the embedded boot* to
  prove embedding doesn't route around anything.
- **One datastore / state vs motion / stateless extensions:** untouched; the ritual is wiring, not
  new behavior.
- **MCP surface:** none — no new tools. This is a Rust API seam, not a platform surface.
- **Data / Bus:** N/A (no new records, no new subjects).
- **Sync / authority / Secrets:** unchanged; `LB_SIGNING_KEY` custody moves into
  `BootConfig::signing_key` filled at the binary boundary — the seed never gets logged, same as today.
- **No mocks:** the embed test boots the real store (`mem://`)/bus/gateway via the lib API — it *is*
  the real path; `test_gateway` refactored onto the seam means the whole UI gateway suite exercises
  it daily.
- **One responsibility per file:** the ritual decomposes per FILE-LAYOUT (config, builder, seeds,
  reactors, serve — a folder of verbs under `rust/node/src/`, not one 400-line `lib.rs`).
- **SDK/WIT impact:** none to the WIT/extension boundary. Flag instead: `BootConfig`/`NodeBuilder`
  becomes the **supported embed API** — additive evolution preferred (builder pattern absorbs new
  fields without breaking embedders).
- **Skill doc:** N/A — a Rust compile-time API, not an agent-/API-drivable surface. The embed
  how-to lands in `public/node-roles/` + the crate's rustdoc, not `skills/`.

## Example flow (third-party embedder)

The first real embedder is planned: **`github.com/NubeIO/rubix-ai`** — a product host/node in a
separate org, git-dep on `NubeDev/lb`, with its own extensions repo (`NubeIO/rubix-ai-extensions`)
consuming the published SDKs per `../extensions/ext-out-of-tree-scope.md`. The flow below is that
repo's story:

1. A Rust service adds `lb-node = { git = "https://github.com/NubeDev/lb", tag = "v0.1.x" }`.
2. ```rust
   let cfg = BootConfig {
       store_path: Some("/var/lib/myapp/lb".into()),
       signing_key: SigningKey::from_seed(&seed),
       gateway: GatewayMode::Listener(my_listener), // or Addr(..) / Off
       reactors: true,
       federation: None,
       ..Default::default()
   };
   let running = NodeBuilder::new(cfg).boot().await?;
   ```
3. `running` hands back the `Node` (store/bus/host verbs) and, if gateway is on, the serve task —
   the embedder calls host verbs in-process (`lb_host::channel_create(...)`) and/or speaks HTTP to
   its own listener.
4. The same caps wall applies: an in-process call still goes through the host's mediated seams; a
   tokenless HTTP call still 401s. Embedding grants nothing.
5. Shutdown: `running.shutdown().await` — reactors stop, sidecars get the supervisor's shutdown,
   store closes. (Today's binaries mostly rely on process exit; the embed API makes teardown real.)

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`), all real (store `mem://`, real bus/gateway):

- **Capability deny (mandatory):** through an embedded boot — a caller without the cap gets
  `Denied` on a host-verb call, and the embedded gateway returns 403/401 exactly as the binary does.
- **Workspace isolation (mandatory):** ws-B cannot read ws-A records on an embedded node (reuse the
  standard isolation test, run via the lib boot path).
- **The load-bearing test — parity:** one integration test boots via `boot_full(BootConfig::from_env())`
  and asserts the same surface the binary exposes (login works, `hello` fixture callable, gateway
  routes live). Plus the **refactor regression net**: the existing UI `pnpm test:gateway` suite (via
  the refactored `test_gateway`) and the desktop shell build both green on the shared ritual —
  they are the proof no seed/reactor was dropped in the move.
- **Subset boot:** gateway-off boot exposes host verbs but no HTTP; reactors-off boot performs no
  background scans (assert via the job/flow tables staying quiet).
- **Teardown:** `shutdown()` leaves no orphan sidecar process and the store reopens cleanly.
- **Feature gate:** default build carries no external-agent code (existing compile-time check keeps
  passing from the lib target).

## Risks & hard problems

- **The ritual is subtly divergent today** — the three copies are *not* identical (Tauri seeds
  differently, test_gateway seeds extra fixtures). Unifying means deciding, per divergence, whether
  it's config (a `BootConfig` field / builder hook) or drift (a bug to fix). Budget real time for
  this archaeology; it is the actual work.
- **API commitment creep.** Once third parties embed, every boot change is API-visible. Mitigation:
  builder + non-exhaustive config struct from day one, additive fields only.
- **Teardown is new ground.** The binaries never needed clean shutdown; embedders do. Sidecar
  shutdown exists (`lb-supervisor`), but reactor tasks and the SSE server need a real cancellation
  path — expect surprises here.
- **Env leakage.** Any `std::env::var("LB_…")` left below the seam silently breaks struct-config
  embedders. A grep-gate (deny `env::var` in `lb-host`/lib boot paths outside `from_env`) keeps it
  honest.

## Open questions

- **Lib/package naming:** ~~keep package `node` with `lib name = "lb_node"`, or rename the package
  `lb-node`?~~ **RESOLVED (embed-node-session, Phase 2a):** package renamed **`lb-node`**, bin stays
  `node`, lib is `lb_node`. All `cargo …-p node` callers repointed to `-p lb-node` (Makefile
  `NODE_BIN`, `deploy/common/Dockerfile`). `version = "0.1.9"` kept (core-skills seeder key).
- **What does `RunningNode` hand back exactly** — the `Gateway` value, join handles, a shutdown
  token, all three? **DECIDED (Phase 2a):** `{ node: Arc<Node>, gateway: Option<(Gateway,
  SocketAddr)>, agent_server: Option<AgentServer> }`, with `RunningNode::serve()` blocking on the
  gateway (no-op when off). Fields are `pub` so a `shutdown()` (reactor cancel + sidecar shutdown
  token) lands additively. **Teardown itself is still deferred** — `serve()` runs to process exit
  today, exactly like the binaries. `GatewayMode::Listener` (`serve_listener` hand-back) also deferred.
- **`#[non_exhaustive]` construction:** the example's `BootConfig { .. ..Default::default() }` struct
  literal is *forbidden* cross-crate by `#[non_exhaustive]` (which is what makes additive fields
  safe). Supported path: `let mut c = BootConfig::default(); c.field = ..;` (all fields `pub`).
- **Role-mount de-env (follow-up):** the core ritual is fully struct-config, but `federation::mount` /
  `control_engine::mount` still read their own `LB_FEDERATION_*` / `LB_CONTROL_ENGINE_*` env. Folding
  those into `BootConfig` (a `FederationConfig`/`ControlEngineConfig` axis) is a bounded next slice.
- **Tag discipline:** do embedders pin the existing `v0.1.x` node-version tags, the `sdk-v*` tags
  from ext-out-of-tree, or a new `node-v*` series? (Recommend: reuse the node binary's existing
  version — it already keys the core-skills seeder.)
- **How much of `full.rs` moves:** the Tauri store-path resolution and window plumbing stay; is the
  federation install/supervise block config or Tauri-specific? (Lean config — the node binary does
  it too.)

## Related

- `node-roles-scope.md` — the roles this ritual selects; §3.1's "thin role-aware layer" doctrine
  this scope leans on for placement.
- `../extensions/ext-out-of-tree-scope.md` — the sibling split: the SDK is its own standalone repo
  (`lb-ext-sdk`/`lb-ext-ui-sdk`) publishing to crates.io/npm, because a downstream consumer needs a
  real library without `lb`-repo access. **Embedding is the opposite call**: `lb-host` is not a
  reusable contract but the whole platform, so it stays a git-dep on `lb` — no separate repo, no
  crates.io publish.
- `../crate-layout/crate-layout-scope.md` — the workspace shape the lib target slots into.
- `../desktop/desktop-packaging-scope.md` — the Tauri shell whose `full.rs` becomes a thin caller.
- `rust/node/Cargo.toml` — the version-keyed core-skills seeding the tag question touches.
- README `§3.1` (thin role-aware layers), `§9` (roles by config), `§3` rules 1/5/6.
