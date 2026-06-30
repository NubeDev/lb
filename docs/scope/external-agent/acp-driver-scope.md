# External-agent scope — the ACP-client driver (`AcpRuntime`)

Status: scope (the ask). Sub-scope #2 of `external-agent-scope.md`. Promotes to `public/external-agent/`.

Implement the **one** `AgentRuntime` impl that drives a third-party agent: `AcpRuntime`. It spawns the
agent as a subprocess, speaks ACP over stdio via the [official Rust SDK](https://github.com/agentclientprotocol/rust-sdk),
bridges **our** MCP server to the agent, and maps the agent's ACP notifications into the canonical
`RunEvent` stream. There is **exactly one driver** — differences between VT Code, dirge, Claude Code, …
are **`AgentProfile` data**, never per-agent code. The safety wrapping (sandbox, built-ins-off) is #3;
this sub-scope assumes #3's hooks and focuses on *talking to the agent correctly*.

## Implementation status — the shipped spike vs the ACP target (read this first)

The **integration wire is ACP** (this scope's plan, below). What exists on disk today is a deliberately
**simpler seam-proof, not the ACP driver**: `rust/crates/external-agent/` drives `vtcode exec --json`
over **NDJSON stdout lines** (no ACP, no SDK dependency — `Cargo.toml` pulls only `lb-run-events`,
`serde`, `tokio`, `thiserror`). It validates the two cheap-to-get-wrong halves — the per-agent
launch/decode seam (`AgentWrapper`) and the wire→`RunEvent` projection — against a **real** binary,
without paying for the ACP transport before the seam is proven worth it.

**These are different wires, and the spike's transport is throwaway.** Moving to ACP is **net-new
work**, not a rename: the JSON-RPC stdio transport, the `initialize` handshake + capability
advertisement, the `-rmcp` MCP bridge, and a decode surface that is `SessionNotification`-based rather
than line-based are all added, not adapted. What **does** carry over unchanged is the
`AgentProfile`-as-data design, the `RunEvent` projection target, and the one-encoder discipline. So:
the stdout `AgentWrapper` layer is the seam-proof; #2 proper **re-points it onto the SDK** and the
NDJSON decode path is retired (or kept only as a non-ACP fallback if a profile opts out of ACP — an
open question, not the default). Treat any "becomes the `AcpRuntime` body with no re-think" phrasing in
the crate's docs as scoped to the *profile + projection*, not the transport.

## Goals

- `AcpRuntime: AgentRuntime` in `lb-role-external-agent` (feature `external-agent`): given a goal, an
  `AgentProfile`, the derived principal, and an MCP endpoint, run the agent to completion emitting
  `RunEvent`s.
- **Spawn + stdio** via `agent-client-protocol-tokio` — launch the profile's binary with its `acp`
  subcommand/args, wire the JSON-RPC stdio transport, drive `initialize` → `session/new` →
  `session/prompt`, and handle `session/cancel`.
- **MCP bridge** via `agent-client-protocol-rmcp` — expose the derived principal's granted MCP tools to
  the agent over ACP, using **the same `rmcp` SDK the node already runs**. A tool the agent proposes is
  dispatched through `lb_mcp` (`caps::check` under the derived principal) and the result returned.
- **ACP → `RunEvent` encode** (`encode.rs`) — the **inverse** of `role/acp/src/encode.rs`: map ACP
  `SessionNotification`s (token / reasoning / tool-call / tool-started / tool-result / done /
  context-overflow) into the canonical `RunEvent` vocabulary (`scope/agent-run/` Part 1), so downstream
  (channel motion, SSE, the job transcript) is identical to the in-house loop.
- **`AgentProfile`** (`profile.rs`) — the swap unit: `{ id, binary, args, acp_version_range, tool_policy
  knobs, granted_tools, persona_skill, model_endpoint_ref, resume_strategy }`. A profile is **not just a
  binary** — it pairs *which agent runs* with *what it can touch* (`granted_tools`, narrowed from the
  derived principal) and *who it is* (`persona_skill`, below). VT Code-as-coder and VT Code-as-data-analyst
  are **two profiles over the same binary**; VT Code and dirge are two profiles over different binaries.
- **Persona via a grant-gated skill (reuse shipped S4).** A profile names a `persona_skill`; at session
  start the driver loads it through the **already-shipped** `assets.load_skill` (the `grant:skill/{id}`
  gate — `public/skills/skills.md`) under the derived principal, and passes the skill body as the agent's
  **initial instructions** over ACP (`session/new`/first `session/prompt`). So the agent's persona is
  itself **workspace-grant-gated** — "this agent is a data analyst" requires the workspace to have granted
  the data-analyst skill. Optionally, `assets.load_skill`/`skill.activate` are included in `granted_tools`
  so the agent can pull further granted skills mid-run (the same model-activated-skills path the in-house
  loop already ships, agent-run Part 5). This is the mechanism that makes a *coding* agent general-purpose:
  **tools + persona are granted data, not code** — see `external-agent-scope.md` ("What a profile is").
- **ACP version negotiation** — pin the profile's expected ACP **protocol** version range; **refuse to
  start** on a mismatch rather than guessing (the SDK client negotiates; we gate). *To verify per
  agent:* the exact ACP protocol version each binary advertises at `initialize` — these are **not** the
  agent's release version (e.g. VT Code's release is `0.133.x`, not an ACP version) and must be read
  from the real handshake, not assumed. The official ACP Rust SDK is the version source of truth.

## Non-goals

- The sandbox / built-ins-off **enforcement** (#3) — this driver *passes the profile's tool-policy
  knobs through* and *calls #3's fail-closed assertion*, but owns neither.
- Model routing (#4) — the driver reads `model_endpoint_ref` from the profile; #4 defines what it
  points at and the token.
- The run **job**, resume execution, and supervision (#5) — the driver emits events and exposes
  `cancel`; persistence/resume/kill are #5.
- Any non-ACP agent — out of scope (not special-cased).

## Intent / approach

**Assemble, don't hand-roll.** The driver is mostly wiring of three Apache-2.0 SDK crates: `-tokio`
(process + transport), the core `agent-client-protocol` (Client role + types), and `-rmcp` (the MCP
bridge to the very SDK we use). Our original code is small: the `AgentProfile` schema, the ACP→`RunEvent`
encoder, and the orchestration that ties prompt → tool-bridge → events together.

**One driver, profiles for difference — plus thin per-agent shims, not per-agent runtimes.** The
temptation is a `VtCodeRuntime` and a `DirgeRuntime` — each its *own loop, own caps handling, own state*;
**reject that** (it re-imports the second-loop anti-pattern). There is **one** `AgentRuntime` impl.
Every per-agent difference (binary, args, the lever that disables built-ins, ACP version, resume
capability) is *data* in an `AgentProfile`, and the small amount that is genuinely *behavioural* — how to
build this agent's argv, how to decode this agent's notification dialect — lives in a **thin, data-ish
per-agent shim** (the shipped spike's `AgentWrapper` impls: `wrappers/vtcode.rs`, `wrappers/codex.rs`).
A shim is **argv + decode only**: no loop, no `caps::check`, no session state, so it does **not**
reintroduce the rejected per-agent runtime. The driver/runtime is single; the shim is a parser. This is
what keeps the seam swappable and the wall test meaningful (one code path, regardless of agent). New
agent = new profile **and** a new ≤1-screen shim file — never a new runtime.

**The encoder is the contract boundary.** Mapping ACP notifications to `RunEvent`s is the one place the
external loop meets our internals; keep it total (every ACP variant maps or is explicitly dropped with a
reason) and pure (no I/O), so live streaming and replay never diverge — the same discipline `agent-run`
already applies to the *outbound* encoder.

**Decline what we won't honor.** During `initialize` the client advertises only the capabilities we
actually back: the MCP tool bridge — and **not** ACP's filesystem/terminal client capabilities (those
are #3's "everything is MCP" stance). Advertising less is how the wall starts at the protocol handshake.

**Persona is a granted skill, not a fork.** A coding-branded agent (VT Code) and a data-analysis agent
are the *same loop* with a different **persona skill** + **granted tool set**. We do **not** fork the
agent or rewrite its prompt in code; we load a workspace-granted skill (shipped `load_skill`, grant-gated)
and feed it as the agent's session instructions, and we grant the data MCP tools (`federation.query`,
`data.query`, `series.*`, `viz.query`) instead of repo tools.

**On "neutralizing" the agent's own persona — best-effort, not a protocol guarantee.** Agents like VT
Code ship a first-class skills system and baked-in system prompts; feeding our skill as `session/new`
instructions makes *our* persona the dominant one, but ACP does **not** guarantee the agent discards its
own system prompt (many agents merge or deprioritize client instructions against their own). So the
*persona* is best-effort. What is **not** best-effort is the **capability** boundary: regardless of what
the agent "thinks it is," it can only call the tools the wall (#3) admits — built-ins denied, granted
MCP tools only, `caps::check` on every call. "General-purpose through our seam" rests on the **tool
grant + wall**, which is enforced; the persona skill steers, the wall constrains. Do not rely on prompt
instructions for safety — only on #3.

## How it fits the core

- **Tenancy / isolation:** the bridge offers **only** the derived principal's granted tools, scoped to
  `ws`; the agent literally cannot enumerate another workspace's tools (proven across store + MCP in #3).
- **Capabilities:** every proposed tool call runs `lb_mcp` → `caps::check` under the derived principal
  (`caller ∩ agent`); ungranted → an ACP tool **error** returned to the agent (it reacts; not a crash).
  No new caller cap (#1).
- **Placement:** `either` — the driver is identical on edge and hub; only the profile (and what the model
  endpoint resolves to) differs.
- **MCP surface:** **consumes** exclusively. The driver exposes no new MCP write verb; it is reached
  *behind* `agent.invoke` via the #1 registry.
- **Data / Bus:** emits `RunEvent`s (motion); persistence is #5. The ACP stdio pipe is **local** motion
  between node and subprocess — never on the Zenoh bus.
- **Secrets:** the driver passes a `model_endpoint_ref` (resolved by #4) — it never sees or forwards a
  provider key.
- **No fake backend (rule 9):** tests spawn the **real** agent binary over a **real** stdio pipe against
  a **real** in-proc MCP server; only the provider HTTP is the existing `MockProvider` (behind the
  gateway, #4). A scripted provider turn is what makes the agent deterministically *propose* a known tool
  call so the bridge + encoder are exercised end-to-end.

### File layout (FILE-LAYOUT)

**Shipped today — the stdout seam-proof (leaf crate, not wired, no ACP):**

```
rust/crates/external-agent/src/
  lib.rs              ← crate root + re-exports (NOT yet an AgentRuntime impl)
  driver.rs           ← drive(wrapper, profile, goal, ws, timeout): spawn + read NDJSON stdout + project
  wrapper.rs          ← trait AgentWrapper { id; command_args; decode_line -> Decoded } (the per-agent seam)
  profile.rs          ← AgentProfile + ModelEndpoint (data: binary, provider/model, api-key env NAME)
  wrappers/
    mod.rs            ← re-exports the shims
    vtcode.rs         ← VtcodeWrapper: `vtcode exec --json` argv + VtcodeEvent NDJSON decode (the reference)
    codex.rs          ← CodexWrapper: FUTURE example (not driven), proves the seam takes a 2nd agent
```

**Integration target — #2 proper, the ACP runtime (role crate, feature-gated, ACP wire):**

```
rust/role/external-agent/src/
  lib.rs          ← AcpRuntime: impl lb_host::AgentRuntime (orchestration)
  spawn.rs        ← launch binary + wire stdio (agent-client-protocol-tokio)
  bridge_mcp.rs   ← offer derived-principal MCP tools to the agent (agent-client-protocol-rmcp)
  encode.rs       ← ACP SessionNotification -> RunEvent (inverse of role/acp/src/encode.rs)
  profile.rs      ← AgentProfile schema (binary, granted_tools, persona_skill, model, resume) + built-in profiles
  wrappers/       ← the per-agent argv/decode shims carry over (re-pointed onto ACP notification decode)
  // sandbox.rs is owned by #3 (capability-wall) and called from spawn.rs
```

The `crates/` leaf becomes the `role/` crate when #1's `AgentRuntime` seam + the `external-agent`
feature land: `driver.rs`'s spawn/collect role is taken over by `spawn.rs`/`lib.rs`, the NDJSON
`decode_line` in each `wrappers/*.rs` is re-pointed onto `encode.rs`'s ACP decode, and `profile.rs`
carries over largely intact. Until then the leaf intentionally lives in `crates/` with no feature and
no dependents (keeps the future feature-OFF build clean — runtime-seam "feature leakage").

## Example flow

1. `AcpRuntime::run(ws, goal, profile=vtcode-default, principal, mcp_endpoint)` is selected by the #1
   registry.
2. `spawn.rs` launches `vtcode acp …` (inside #3's sandbox) and wires stdio; the client `initialize`s,
   advertising the MCP bridge and **declining** fs/terminal caps; ACP version is checked against the
   profile range (mismatch → refuse to start).
3. The driver loads the profile's `persona_skill` via `assets.load_skill` (grant-gated, derived
   principal); `session/new` carries that skill body as instructions, then `session/prompt { goal }`.
   The agent prompts its model via the gateway (#4).
4. The agent proposes `data.query{…}`. `bridge_mcp.rs` runs it through `lb_mcp` → `caps::check`
   (derived principal). Granted → executed, result returned over ACP; ungranted → ACP tool error.
5. Each ACP `SessionNotification` → `encode.rs` → a `RunEvent`; #5 streams + persists them.
6. The agent emits `done` (or a cancel arrives) → `run` returns; #5 marks the job.

## Testing plan

- **Real-pipe integration (rule 9):** spawn the **real** default agent binary; a scripted provider turn
  makes it propose a granted tool; assert the bridge dispatched it through `caps::check` and the result
  round-tripped — over a real stdio ACP pipe, real MCP, real store (`mem://`).
- **Encoder unit tests:** every ACP `SessionNotification` variant maps to the right `RunEvent` (or is
  explicitly, reasoned-ly dropped); totality test so a new ACP variant fails loudly.
- **Version negotiation:** a profile pinned to an incompatible ACP version **refuses to start** (no
  silent downgrade).
- **Swap test (umbrella gate):** re-run the integration with the **dirge** profile — same driver code,
  no change — proving profiles, not adapters, carry the difference.
- **Capability-deny (§2.1):** the agent proposes a tool the derived principal lacks → ACP tool error,
  effect never happens. (Full wall coverage incl. built-ins-off is #3.)
- **Persona is grant-gated:** a profile whose `persona_skill` is **not granted** in the workspace fails
  the run at session start (the shipped `load_skill` grant gate denies) — the persona can't be smuggled
  in without the workspace granting it. **Profile reuse:** the *same* binary with a coding skill +
  repo tools vs a data skill + `federation.query`/`data.query` produces a coder vs a data analyst — two
  profiles, one binary, no code change (the "general-purpose" proof).

## Risks & hard problems

- **ACP dialect drift between agents.** Versions and optional-capability support differ; the encoder must
  tolerate unknown notification fields and the client must negotiate a pinned range. A profile that lies
  about its version is caught at handshake.
- **Tool-call id correlation.** Some providers emit empty tool-call ids (dirge handles this with a
  correlator); the bridge must correlate request↔result robustly (stable id, FIFO fallback) or results
  attach to the wrong call.
- **Encoder partiality.** A missed ACP variant silently drops progress; enforce totality in tests.
- **Stdout pollution.** The agent must speak clean JSON-RPC on stdout; banner/log noise on stdout breaks
  the transport. Profiles must select the agent's machine/`acp` mode, not its TUI.

## Open questions

- **Tool exposure breadth:** offer the full granted catalog or a per-profile curated subset (never
  widened)? Default: derived-principal granted tools, optionally narrowed by the profile.
- **CI binary availability:** build a pinned agent crate in CI, or ship a tiny **real** ACP agent built
  from the SDK examples as the deterministic counterpart (tests the seam without faking *Lazybones*)?
  Decide which is the gate vs the nightly. (Shared with #3.)
- **`AgentProfile` storage:** deploy config (this slice) vs a ws-scoped record + `agent.profile.*` CRUD
  later.

## Related

- `external-agent-scope.md` (umbrella), `runtime-seam-scope.md` (#1, registers this),
  `capability-wall-scope.md` (#3, the sandbox `spawn.rs` calls), `model-routing-scope.md` (#4, the
  `model_endpoint_ref`), `run-lifecycle-scope.md` (#5, persistence/resume/supervision).
- `scope/agent-run/agent-run-scope.md` — the `RunEvent` vocabulary + `role/acp/src/encode.rs` (the
  outbound encoder this mirrors); Part 5 = the model-activated-skills path reused for `granted_tools`.
  `scope/mcp/mcp-scope.md`, `scope/auth-caps/auth-caps-scope.md`.
- `public/skills/skills.md` (**shipped S4**) + `scope/skills/skills-scope.md` — the grant-gated
  `load_skill` this loads the `persona_skill` through; the persona is workspace-grant-gated, not forked.
- External: official ACP Rust SDK (`agent-client-protocol`, `-tokio`, `-rmcp`).
