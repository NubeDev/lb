# Session — external-agent runtime-seam (#1) built + real `exec --json` driver wired behind it

Date: 2026-07-01
Topic: `docs/scope/external-agent/` (sub-scope #1 runtime-seam, integrating the shipped #2 spike)
Status: **#1 shipped end to end** — the `AgentRuntime` seam + registry + `external-agent` cargo
feature are real (not stubs), and the existing `exec --json` driver is reachable through them.
Open Interpreter is the default external agent; VT Code and Codex are swappable by profile id with
no code change. #3/#4/#5 are named, linked seams with TODOs (not faked).

## The ask (this session)

Wire the external-agent path into the node so `agent.invoke { runtime, goal }` can drive a real
external agent subprocess, with **Open Interpreter the default** and **VT Code/Codex swappable by
config, no code change**. Build #1 for real, integrate the existing `exec --json` driver behind it,
and leave #3/#4/#5 as named seams. Keep the shipped NDJSON transport (do NOT do the ACP SDK swap
this session — recorded as the next slice).

## What shipped

### 1. `AgentRuntime` seam — host-owned, beside `ModelAccess` (real)

`rust/crates/host/src/agent/`:
- `runtime.rs` — the **`AgentRuntime` trait** (object-safe: `run(&self, node, RunContext) ->
  Pin<Box<dyn Future<..>>>`), `RunContext` (ws + goal + caller principal + agent_caps + MCP endpoint
  `&Node` + tools + ts), and `ErasedModel` — an object-safe erasure of `ModelAccess` (whose `turn`
  returns `impl Future`, so it is not object-safe). `ModelHandle` round-trips an `ErasedModel` back
  into a `ModelAccess` so the in-house loop is **both** a registry trait object and a `ModelAccess`
  consumer, with **no second loop**. Mirrors the `ModelAccess`/`Provider` move exactly, one level up.
- `in_house.rs` — `InHouseRuntime: AgentRuntime`, the **always-registered default** (`id="default"`).
  Its `run` calls the existing `run_session` verbatim through the seam (no new path). This is the
  "blanket-impl the in-house loop as the default" the scope asks for.
- `registry.rs` — `RuntimeRegistry`: id → `Arc<dyn AgentRuntime>`. `with_default(model)` builds the
  default-only (feature-OFF) registry; `register(rt)` is additive. **Resolution (decided):** absent →
  default; known id → entry; **explicitly-named unknown → error** (never a silent downgrade). `ids()`
  feeds the future `agent.runtimes` (#5, TODO). Lookup, not a `match`.
- `dispatch.rs` — `invoke_via_runtime(...)`: the ONE selection point. Runs the invoke gate
  (`mcp:agent.invoke:call`, workspace-first) **identically for every runtime** (choosing a runtime is
  an argument, not a grant), resolves the registry, bakes substrate (skill/doc) into the goal **only**
  for the default runtime (external agents load their persona themselves via grant-gated `load_skill`,
  #2), and dispatches through the trait object. `Substrate { skill, doc }`.

### 2. The `exec --json` driver adapted behind the seam (feature-gated role crate)

`rust/role/external-agent/` (new crate `lb-role-external-agent`, pulled ONLY behind the feature):
- `lib.rs` — `AcpRuntime: lb_host::AgentRuntime`. **One** runtime type; per-agent difference is data.
  Its `run`: (a) seals a **per-run scratch dir** as cwd (the run-lifecycle #5 filesystem seal — the
  `drive(.., workspace, ..)`-treats-workspace-as-cwd gap, closed now), (b) calls the existing
  `lb_external_agent::drive(wrapper, profile, goal, scratch_cwd, timeout)`, (c) forwards each projected
  `RunEvent` via `publish_run_event` (a watcher sees an external run identically to an in-house one),
  (d) returns the answer with a **fail-closed** terminal-outcome check (unknown status → Failed).
  `register(&mut registry, model, scratch_base)` — the registration hook.
- `profiles.rs` — the **swap unit**: `resolve_builtin(id)` → `(AgentProfile, Box<dyn AgentWrapper>)`.
  `open-interpreter-default` → binary `interpreter` + codex-family shim; `codex-default` → binary
  `codex` + **same** shim (zero-code alternate); `vtcode-default` → binary `vtcode` + vtcode shim.
- `scratch.rs` — `ScratchDir::create(base, ws, job_id)` → `{base}/lb-external-agent/{ws}/{job_id}`,
  with path-component sanitization so a hostile ws/job id can't escape `base`. Distinct dir per run,
  per ws (the zero-cross-ws-bleed filesystem seal; #3 adds the kernel confinement of this same dir).

### 3. The `external-agent` cargo feature (OFF by default) + registration hook

- `rust/node/Cargo.toml` — `lb-role-external-agent` is an **optional** dep; feature
  `external-agent = ["dep:lb-role-external-agent"]`, `default = []`.
- `rust/node/src/external_agent.rs` — `register_external_runtimes(&mut RuntimeRegistry, model,
  scratch_base)`, `#[cfg(feature = "external-agent")]`. With the feature off it is absent and the role
  crate is not compiled. (Not yet invoked from `main`: `serve_agent` is not booted in `main` today —
  documented serve-wiring TODO; the registration path is the compiled, tested one.)
- **Selection wired into the routed `agent.invoke`:** `AgentInvokeRequest` gained `runtime:
  Option<String>` (`#[serde(default)]`, backward-compatible wire); `serve_agent` now takes an
  `Arc<RuntimeRegistry>` and dispatches via `invoke_via_runtime`; `invoke_remote` gained a `runtime`
  arg. So `agent.invoke { runtime: "open-interpreter-default" }` routes through the registry to the
  real `exec --json` driver; absent → default; unknown → error.

## Decisions made (and the DEFAULTs taken)

- **Unknown-runtime resolution:** explicitly-named unknown → **error** (`BadInput` naming the id);
  absent → default. (Matches the runtime-seam open-question proposal — decided.)
- **Object-safety via `ErasedModel`** (DEFAULT/most-reversible): the trait must be object-safe for a
  `Box<dyn AgentRuntime>` registry, but `ModelAccess::turn` returns `impl Future`. Rather than change
  `ModelAccess` (churns the gateway bridge), I erased it behind `ErasedModel` + `ModelHandle`. Nothing
  about `ModelAccess` or the gateway changes.
- **Substrate only for the default runtime** (DEFAULT): the in-house loop's skill/doc baking stays on
  the default path; external agents load persona via their own grant-gated `load_skill` (#2), so the
  seam does not smuggle in-house substrate into an external goal.
- **`serve_agent` carries the registry** (not a per-invoke build): the registry is built once by the
  wiring layer (`with_default(model)` + feature-gated `register`) — feature + config decides its
  contents, never a branch. This is where the routed `agent.invoke` resolves `runtime`.
- **Scratch dir now, unbounded per-ws concurrency, no per-ws lock** (per run-lifecycle DECIDED): each
  run constructs its own `ScratchDir`; nothing shared between runs. No serialize/queue.
- **Kept the `exec --json` transport** (per the ask): the ACP SDK swap is the next slice, additive.
  Recorded in the crate docs + the acp-driver scope's "Implementation status".
- **`AcpRuntime` name kept** even though the wire is `exec --json` this slice — the type's role in the
  seam is unchanged by the eventual ACP transport swap.

## Explicitly NOT built this session (named, linked seams — not faked, not half-built)

- **#3 capability-wall / OS sandbox + built-ins-off (the safety gate):** the scratch-dir cwd seal is
  in; the kernel egress/fs confinement + fail-closed built-ins-off assertion are TODO(#3) in
  `AcpRuntime::run` / `capability-wall-scope.md`. Nothing that drives a real external agent *in anger*
  should ship before #3 — this slice's live run is opt-in only.
- **#4 model-routing / served OpenAI-compat endpoint + scoped token:** `ModelEndpoint` is config data
  the profile carries; the gateway does not yet *serve* an OpenAI face (blocking prereq, no owner).
  TODO(#4).
- **#5 durable job / resume / supervision:** `run` emits `RunEvent`s a #5 job would persist; the run
  here is collect-then-forward, not a supervised durable job with resume. TODO(#5). The scratch-dir
  seal (a #5 "code gap today" item) *was* addable + testable now, so it landed.
- **`agent.runtimes` read verb (#5):** `RuntimeRegistry::ids()` exists; the MCP verb is a TODO.

## Tests (rule 9 — no mocks; offline projection/registry/resolution are the gate)

Host seam gate — `rust/crates/host/tests/agent_runtime_seam_test.rs` (real Node + MockProvider):
- absent runtime → in-house default runs through the seam and returns its answer;
- explicitly-named `default` resolves the same;
- explicitly-named unknown runtime → **error** naming the id (no silent downgrade);
- **capability-deny (§2.1):** invoke without `mcp:agent.invoke:call` → `Denied`, before selection;
- default-only registry lists only `default`.

Role crate — `rust/role/external-agent/tests/swap_test.rs` (offline config/registry):
- **swap proof (umbrella gate):** open-interpreter vs vtcode = different binary + wrapper, one code
  path; open-interpreter vs codex = **same** wrapper, only the binary differs;
- `register` populates the registry with all three built-ins + default; each resolves to an
  `AcpRuntime` whose `id()` echoes the profile id (one type behind every entry);
- default-only registry has no external entries; `AcpRuntime::new` is `None` for an unknown id;
- **workspace-isolation (scratch seal):** two ws → disjoint dirs; two runs same ws → separate dirs;
  a hostile ws/job id **cannot escape** `base`.

Opt-in real-subprocess — `rust/role/external-agent/tests/smoke_test.rs` (gated `EXTAGENT_SMOKE=1`):
boots a real Node, builds the default external `AcpRuntime`, drives the real binary through the seam,
asserts a non-empty answer (fails loud on an empty stream). No-ops (green) otherwise. Z.AI GLM-4.6 via
`ZAI_API_KEY` + the coding endpoint (verified this topic; not re-probed here).

## Compile-time optionality gate (the headline #1 gate) — verified

- `cargo build --workspace` (feature OFF) — green.
- `cargo build -p node --features external-agent` (ON) — green.
- `cargo tree -p node -e no-dev | grep external-agent` → **NONE** (ACP/external deps absent from the
  OFF build). `cargo tree -p node --features external-agent` → lists `lb-external-agent` +
  `lb-role-external-agent`. So the OFF build carries none of the external code.

## Exact commands (all run this session)

```
cd rust
cargo build --workspace                                  # feature OFF — green
cargo build -p node --features external-agent            # feature ON  — green
cargo tree -p node -e no-dev | grep external-agent       # NONE (OFF-build deps-absent gate)
cargo tree -p node --features external-agent -e no-dev | grep external-agent   # both listed
cargo test -p lb-host --test agent_runtime_seam_test     # 5 passed
cargo test -p lb-role-external-agent                     # 9 + 1(smoke no-op) passed
cargo test --workspace                                   # green (see output in session)
cargo fmt && cargo fmt --check                           # clean
```

## Next slice

- **#2 proper:** move the default onto the official ACP SDK (`agent-client-protocol{,-tokio,-rmcp}`)
  over `interpreter acp` (verified reachable: initialize, protocolVersion 1, loadSession:true). The
  SDK deps are added in `role/external-agent`, still behind the same feature — the OFF build stays
  clean. The seam (`AgentRuntime`, `RunContext`, registry) is unchanged; only `AcpRuntime`'s transport.
- **#3 the wall** before any in-anger external run: OS sandbox around the scratch dir + fail-closed
  built-ins-off assertion in `AcpRuntime::run`.
- **Boot wiring:** invoke `serve_agent(node, Arc::new(registry), agent_caps)` where the gateway wires
  the agent surface, calling `node::external_agent::register_external_runtimes` under the feature.
