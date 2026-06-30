# Session — vtcode external-agent driver spike

Date: 2026-06-30
Topic: `docs/scope/external-agent/` (sub-scopes #1 runtime-seam, #2 acp-driver)
Status: spike shipped — standalone `lb-external-agent` crate, **not yet integrated** into the node.

## The ask

"vtcode is installed — see how good it really is, and start on a crate/API to it, but not
integrated yet." So two halves: (a) evaluate vtcode as an external-agent candidate; (b) lay down the
first real Rust toward the external-agent topic without wiring it into the node.

## What vtcode actually is (evaluation)

`vtcode 0.133.23` (MIT, Vinh Nguyen). It is a full ACP-capable coding agent with several
machine-facing surfaces relevant to driving it as a subprocess:

- `vtcode acp [zed|standard]` — Agent Client Protocol bridge over stdio. This is the surface the
  topic's #2 (acp-driver via the official ACP Rust SDK) ultimately targets.
- `vtcode exec --json` — headless run, **newline-delimited JSON event stream** on stdout. This is the
  surface this spike drives (no extra SDK crate needed to prove the seam).
- `vtcode ask` — single prompt, no tools. Proven to reach a provider end-to-end.
- `vtcode schema tools`, `vtcode models`, `vtcode mcp`, `vtcode a2a`, `vtcode review` — rich
  introspection + MCP + Agent2Agent surfaces, all consistent with the topic's "tools = our MCP" plan.

Verdict: a strong default candidate exactly as the umbrella scope names it. The `exec --json` stream
maps cleanly onto our `RunEvent` vocabulary, and `acp` gives the eventual SDK-driven path.

### Operational gotchas found (documented so the next session doesn't re-discover them)

1. vtcode **resets the shell cwd** to the launch dir after each run; write config with an absolute
   `--output` path, not a relative `./vtcode.toml`.
2. `exec` requires, in order: workspace **trust** (`VTCODE_TRUST_WORKSPACE=full-auto` or a config
   trust entry), `[automation.full_auto].enabled = true` in `vtcode.toml`, and a resolvable model.
3. The model for `exec` resolves via `[agent] provider/default_model/api_key_env`; if `api_key_env`
   points at an unset var the model silently falls back to the sentinel `inherit` and errors. Set
   `api_key_env` to the var that actually holds the key.
4. `vtcode config` prints an ANSI-coloured `[CONFIG]` banner + "Generated configuration:" preamble to
   stdout but **also writes the file**; consume the file, not the captured stdout.

### Not fully demonstrated live: provider quota

The provided zai-coding key authenticates and the model id is accepted (`glm-4.6`/`glm-5.2`), but the
endpoint returned `Rate limit exceeded` on every call across the session, including after multi-minute
backoffs — i.e. a hard quota/throttle on the key, not a transient burst. The driver + decode + project
path is therefore proven by the offline projection tests and the spawn path; the live end-to-end run
is the opt-in smoke test (`VTCODE_SMOKE=1`), ready to go green the moment a non-throttled key is used.

Running the smoke test for real (`VTCODE_SMOKE=1`) confirmed the **spawn + stdout-read + timeout +
bracketing** path end to end against the real binary: `drive(..)` spawned vtcode, the process ran to
completion, and the collected stream was exactly `[RunStart]` — vtcode hit the rate limit and emitted
no NDJSON. The test asserts a text-or-finish event and so **fails loud** on that empty stream rather
than passing on nothing; that strictness is intentional and kept. Net: every part of the driver works
against a live binary; only the model tokens are missing.

## What shipped (the crate)

`rust/crates/external-agent/` — a **leaf** crate nothing else depends on (keeps the future OFF build
clean per runtime-seam "feature leakage"):

- `profile.rs` — `AgentProfile` + `ModelEndpoint`: pure config data (binary, provider/model, the
  *name* of the API-key env var — never the key value). `AgentProfile::vtcode_default(..)`.
- `wrappers/vtcode.rs` — the **one** file that knows vtcode's `exec --json` JSON. Tolerant
  `#[serde(tag = "type")]` enum with `#[serde(other)] Other`, and `project(turn) -> Vec<RunEvent>`
  mapping onto the platform's single `lb_run_events::RunEvent` vocabulary (their wire → our RunEvent,
  the mirror of the role-crate "thin encoder, own file" rule).
- `driver.rs` — `drive(wrapper, profile, goal, workspace, timeout)`: spawns the real binary, reads
  the stream,
  brackets the stream with `RunStart`, projects each line, kills on a liveness timeout. Unparsable
  lines are skipped (logged), never fatal.

### Refactor: the `AgentWrapper` seam (vtcode is the one real agent; codex is a *future* example)

Per the ask ("refactor so we could add codex or pi.dev, easy to change later"), the agent-specific
knowledge moved behind a one-method-pair trait so the driver is agent-agnostic:

- `wrapper.rs` — `trait AgentWrapper { id; command_args(profile, goal, ws); decode_line(line, turn)
  -> Decoded }`. `Decoded` is `Message | Events | Ignore` so a wrapper says "this line was a model
  message" (driver bumps the turn) without re-implementing turn numbering.
- `wrappers/vtcode.rs` — the **shipped, exercised** impl (`VtcodeWrapper` + the `VtcodeEvent` NDJSON
  shape). This is the only agent driven against a real binary in this spike.
- `wrappers/codex.rs` — a **future** example (`CodexWrapper`, codex's nested `msg.type` JSONL). Codex
  is **not** an integration target yet — it exists so the design is *proven swappable*: a second agent
  is a new file implementing the same trait, with **no driver change**. Its event-field names are
  best-effort against the documented codex schema and have **not** been driven against a real codex
  binary; treat it as a worked example to reconcile when/if codex is actually wired, not as shipped
  support.
- `driver.rs::drive(wrapper, profile, goal, ws, timeout)` — now generic over `&dyn AgentWrapper`.
- `profile.rs` — `vtcode_default(..)` + `codex_default(..)`; the smoke test picks wrapper+profile by
  `VTCODE_AGENT` env, so the **same call site** *could* drive either agent (the umbrella "swap test").

The point of carrying codex now is to make sure the seam *accounts for* a second, structurally
different agent (nested payload, exit-code-as-error, no `--api-key-env`) before we commit to vtcode's
shape — not to ship codex. Adding pi.dev later is the same move: `wrappers/pi.rs` + a `pi_default`
profile + one re-export. Nothing else.

### Deliberately NOT here (later sub-scopes)

The `AgentRuntime` host trait + `external-agent` cargo feature (#1), the capability wall / built-ins
-off sandbox (#3), gateway model routing (#4), and the durable job / resume / supervision (#5). This
spike is the *wire + seam* proof only; `drive(..)`'s shape is what the #1 trait adapts onto.

## Tests (green)

- `tests/projection_test.rs` — 8 unit tests over the decode+project boundary (text, reasoning, tool
  call → start+args, tool-result ok vs **error/deny**, every terminal status word, unknown-type
  tolerance). No model, no subprocess. Rule-9 compliant (a parse boundary, not a fake backend).
- `tests/vtcode_smoke_test.rs` — opt-in **real-subprocess** test gated on `VTCODE_SMOKE=1`; no-ops
  (green) otherwise so default `cargo test` stays offline. Spawns a genuine `vtcode exec --json`.

`cargo test -p lb-external-agent` → 8 passed + smoke (skipped/no-op). `cargo build --workspace` green.

## Peer review applied (scope reconciliation)

A peer review of `docs/scope/external-agent/` flagged that the scope's "the wire is ACP" thesis
disagreed with the shipped stdout spike, plus several factual/citation issues. Changes made (agreed
with the review on all substantive points):

- **ACP vs stdout (blocking):** added an "Implementation status" block to `acp-driver-scope.md` and
  rewrote the crate's `lib.rs` doc — the spike is an explicit **seam-proof over NDJSON stdout**, not the
  ACP driver; the transport is throwaway and #2 **re-points** the wrappers onto the ACP SDK
  (`initialize` handshake, `-rmcp` bridge, `SessionNotification` decode are net-new). Only the
  `AgentProfile`-as-data + `RunEvent` projection carry over.
- **FILE-LAYOUT (blocking):** `acp-driver-scope.md` now shows the **real** spike layout
  (`crates/external-agent/...` + `wrappers/`) alongside the `role/` integration target, and how one
  becomes the other.
- **"No per-agent code" (blocking):** reworded to "one runtime, thin per-agent **argv+decode shims**
  (not per-agent runtimes)" — a shim has no loop/caps/state, so it doesn't re-import the anti-pattern.
- **Capability wall (high):** verified VT Code's real levers against the binary (`tool-policy deny-all`
  + fail-closed `[automation.full_auto].allowed_tools` + `acp --allowed-tools/--disallowed-tools`);
  marked **dirge `--no-tools` as unverified/likely-fictional**; elevated the **ACP advertisement**
  (client controls offered tools) to the *primary* tool lever, CLI flags as defence-in-depth.
- **Persona (high):** softened to **best-effort** — the persona skill steers, the **wall constrains**;
  safety rests on the tool grant + wall (#3), not prompt instructions.
- **Model-routing (medium):** elevated the gateway's **served** OpenAI-compatible endpoint
  (agent→gateway, distinct from the existing consumed provider contract) to a blocking, owner-needed
  cross-scope prerequisite.
- **Versions/citations (medium/low):** removed the stale `VT Code 0.10.4` (real release `0.133.23`),
  reframed ACP version as the *protocol* version to verify at handshake; fixed `§6.13`→`§6.14`/`§6.16`.
- **Codex (medium):** documented in the umbrella as a **future** example (not an integration), dirge as
  the GPL ACP alternate.
- **Fail-closed outcome (low) — code change:** `outcome_of` now maps an **unrecognised** vtcode status
  to `Failed` (not `Done`) — an untrusted agent's unknown terminal word must not read as success; an
  *absent* status stays `Done` (normal completion). Added a regression case + a `#5` testing-plan line
  that the **job/exit** is authoritative over the self-reported word.

## Open Interpreter evaluated → codex-family wrapper, verified schema

Evaluated `openinterpreter/openinterpreter` (cloned to `/tmp/openinterpreter`) as an alternative:

- **What it is:** Apache-2.0, a **Rust fork of OpenAI Codex** (`codex-rs/`), binary `interpreter`,
  positioned as "a coding agent for low-cost models" with harness emulation. License is clean (better
  than dirge GPL-3.0, on par with vtcode MIT).
- **ACP-native:** `codex-rs/acp-server` implements `acp::Agent` (initialize/session/prompt/cancel/
  permission + `AgentCapabilities`) — so it fits the topic's "wire is ACP" integration directly, and
  the wall's *primary* tool lever (client controls advertised tools) applies.
- **Codex-compatible:** because it's a Codex fork, its `exec --json` and ACP surfaces are Codex-shaped.
  So **one shim drives the whole codex family** — Codex and Open Interpreter differ only by
  `AgentProfile.binary`. Added `AgentProfile::open_interpreter_default` (binary `interpreter`) reusing
  `CodexWrapper` unchanged; a test asserts the swap is *only* the binary. This is the cleanest possible
  demonstration of the seam: a second agent for **zero** new code.

**Verified the real schema (fixes a peer-review gap).** The earlier `CodexWrapper` *guessed* a
`{"msg":{"type":"agent_message"}}` shape — wrong. Read the actual source
(`codex-rs/exec/src/exec_events.rs`) and rewrote the wrapper to the real `ThreadEvent`/`ThreadItem`
model: top-level `thread.started` / `turn.started|completed|failed` / `item.started|updated|completed`
/ `error`, where `item.*` carries `ThreadItem { id, <details tagged by type> }` (`agent_message`,
`reasoning`, `command_execution` with `exit_code`, `mcp_tool_call`, …). Tests rewritten to the real
schema; 10 unit tests green. The codex-family wrapper is now schema-accurate, not best-effort.

**Recommendation:** Open Interpreter is a strong *default* candidate (Apache-2.0 + ACP-native +
Codex-compatible + low-cost-model focus). VT Code stays the exercised default until one of these is
driven against its real binary over the ACP path; recorded in the umbrella scope as the leading future
option.

### Open Interpreter RUN FOR REAL against Z.AI GLM-4.6 (now exercised, not just source-verified)

Installed `interpreter 0.0.17` and drove a real `exec --json` run against **Z.AI GLM-4.6**:

- **Provider wiring:** Z.AI is OpenAI-compatible; configured via Codex's standard `model_providers`
  (`-c` overrides). The **coding-plan** base URL `https://api.z.ai/api/coding/paas/v4` works and is
  **not throttled**; the built-in zai provider (env `ZHIPU_API_KEY`, standard endpoint) returned
  `429 Too Many Requests` — same hard quota cap the vtcode runs hit. **Use the coding endpoint.**
- **Result:** the run returned `PONG`. Full real stream:
  ```
  {"type":"thread.started","thread_id":"…"}
  {"type":"item.completed","item":{"id":"item_0","type":"error","message":"Model metadata for `glm-4.6` not found. Defaulting to fallback…"}}
  {"type":"turn.started"}
  {"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"PONG"}}
  {"type":"turn.completed","usage":{"input_tokens":10750,…,"output_tokens":5}}
  ```
- **Wrapper validated against the real wire:** the live stream matches the source-derived schema
  exactly. One refinement from observation — a **non-fatal** `item.completed` with `type:"error"` (the
  glm-4.6 metadata warning) is now a modelled `Details::Error` variant and surfaces as a failed
  tool-result instead of being dropped. Added `codex_wrapper_projects_a_real_open_interpreter_stream`
  built from the captured lines. **11 unit tests green.**

So the codex-family wrapper is now **exercised against a real binary + real provider**, not just
source-verified — and the answer to "will it work with Z.AI?" is **yes**, via the coding endpoint.

### Decision: Open Interpreter is now the DEFAULT external agent (+ ACP verified)

Promoted Open Interpreter from "leading candidate" to **the default external agent** across the scope
(`external-agent-scope.md` thesis/External, `acp-driver-scope.md` example flow + implementation status,
`run-lifecycle-scope.md` resume note; `profile.rs` doc). Rationale, all evidence-backed:

- Apache-2.0 (clean license); explicitly low-cost-model focused (platform fit).
- **Ran a real agentic coding task vs Z.AI GLM-4.6** — wrote `hello.py` (disk usage), ran it,
  self-corrected `python`→`python3`, reported real output. Exercised every `CodexWrapper` branch.
- **ACP wire VERIFIED:** `interpreter acp` answers a real `initialize` →
  `{protocolVersion: 1, agentCapabilities: {loadSession: true, mcpCapabilities, sessionCapabilities:
  {list, close}}, authMethods: […]}`. So the topic's "wire is ACP" thesis holds for the default, and
  **`loadSession: true`** means ACP `session/load` resume is available for the default (eases #5).

**Does this affect ACP / the scope?** No — it *strengthens* it. ACP stays the integration wire; Open
Interpreter is ACP-native (verified), so nothing about the ACP plan changes. VT Code and Codex remain
fully-supported alternates (VT Code = the `wrappers/vtcode.rs` shim; Codex = the same
`wrappers/codex.rs` as the default). The only change is which profile is selected by default. The
shipped spike still uses `exec --json` (the seam-proof transport); #2 moves the default onto the ACP
SDK, now known-reachable.

## Next

- When a non-throttled provider key is available, run the smoke test and **capture the real NDJSON**;
  reconcile `vtcode_event.rs`'s field names against the observed schema (the one file to touch).
- Build #1 (`AgentRuntime` seam + feature) and adapt `drive(..)` into the `AcpRuntime` body.
- Move from `exec --json` to the official ACP SDK over `vtcode acp standard` for the full #2 surface.
