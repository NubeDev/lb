# Agent-run — a streamable, externally-drivable, interactively-gated run (session)

- Date: 2026-06-28
- Scope: ../../scope/agent-run/agent-run-scope.md
- Stage: S10 (platform maturity / coding-agent workplace) — builds on S5 agent + S6 workflow.
- Status: what-shipped (Parts 0–5 all built + tested green)

## Goal
Make the agent run a first-class, **observable, interruptible, replayable** object: a durable typed
transcript, one `RunEvent` vocabulary derived from it, per-tool-call Allow/Deny/Ask with durable
first-settle suspension, an `agent.watch` SSE feed, an ACP stdio adapter, and model-activated skills.
Build order followed the scope's peer-review ordering exactly: **Part 0 → 1 → 3 → 2 → 4 → 5** (built
0,1 first as the foundations everything imports; then fanned out 2 + 5 as sub-agents while building
3 inline; then 4 as the capstone).

## What shipped (all parts, all green)

### Part 0 — durable typed run state (the prerequisite)
- **`lb_jobs::TranscriptEvent`** (`crates/jobs/src/transcript.rs`): a `#[non_exhaustive]`,
  `#[serde(tag="kind")]` enum — `AssistantTurn` / `ToolCallProposed{id,name,args}` / `ToolResult` /
  `SkillActivated` / `SuspensionOpened` / `SuspensionSettled`. Replaces the opaque `Step.result:
  String`. `Job` gained `schema_version` (versioned from day one, serde-default to 1) and two new
  `JobStatus` variants `Suspended` (resumable) + `Cancelled` (terminal), with `is_resumable()`.
- New verbs (folder-of-verbs): `append_event` (replaces `append_step`), `cancel`, `suspend`/`unsuspend`.
- **`lb_store::create`** (`crates/store/src/create.rs`): the conditional first-write (SurrealDB
  `CREATE`, errors `StoreError::Conflict` on an existing id) — the first-settle primitive Part 2 needs.
- **`rehydrate`** (`crates/host/src/agent/rehydrate.rs`): folds the durable transcript back into the
  exact `messages` + `prior` + `active_skills` the live loop held. `run_session` now rehydrates on
  resume **instead of re-deriving from the goal** (the old `run.rs:68` bug). Cancel hook = `cancel_run`.
- Gated FIRST by its own test: a run reloaded from the store **continues the conversation, does not
  re-ask** (`agent_rehydrate_test.rs`).

### Part 1 — the `RunEvent` vocabulary (event-sourced)
- New low-level crate **`lb-run-events`** (`crates/run-events/`) — depends only on `lb-jobs` + serde
  (NO protocols/gateway/wasm). `RunEvent` (TextDelta, ReasoningDelta, ToolCallStart,
  **ToolCallArgsDelta** for AI-SDK from day one, ToolCallResult, SkillActivated, Suspended, Settled,
  RunFinish) + `RunOutcome`. `project(job)` = snapshot; `project_one(event, turn)` = live deltas.
- Unit-pinned: **live and a `session/load` replay yield the identical view** (review point 5).

### Part 3 — `agent.watch` + SSE + the start/resume-vs-watch split
- **`crates/host/src/run_events/`** (folder-of-verbs): `publish_run_event` (the loop emits motion
  after each durable append, best-effort), `watch_run` (`agent.watch` — a transcript snapshot +
  live `RunEventSub`), `run_subject` (`ws/{id}/run/{job}/events`, workspace-walled by `lb_bus`).
- The loop (`run.rs`) gained an `emit()` helper = append + publish, plus terminal RunFinish emission.
- Gateway SSE route **`GET /runs/{job}/stream?token=`** (`role/gateway/src/routes/run_stream.rs`,
  mirrors `channel_stream`): snapshot-then-deltas, `mcp:agent.watch:call` gated, ws-walled.
- `agent.invoke` kept as the compatibility wrapper; the lifecycle path is start/resume + watch.

### Part 2 — per-tool Allow/Deny/Ask + first-settle Ask (sub-agent, integrated)
- **`crates/host/src/agent/policy/`** — `agent_policy:{ws}` record, pure evaluator (glob on tool name
  + shallow arg-path equality, **Deny > Allow > Ask**, default-allow), `agent.policy.set` (admin cap).
- **`crates/host/src/agent/decision/`** — the dedicated `agent_decision:{job}:{tool_call}` record:
  `open` does `lb_store::create` (reserves the key, first-write), `settle` does a guarded
  Pending→Settled flip, `resume` applies Deny / Allow→replay from the persisted args. `agent.decide`
  verb (`mcp:agent.decide:call`). The Ask still surfaces a `needs:approval` inbox item for routing —
  but the binding settle is the first-write record, **not** the last-writer-wins `Resolution`.
- The loop consults the policy in front of `caps::check`; an Ask suspends durably and ends the turn.

### Part 5 — model-activated skills (sub-agent, integrated)
- `list_granted_skills` / `SkillCatalogEntry` (title+description only) + `render_catalog` inject the
  granted-skills catalog once per run. `skill.activate` is a **loop-internal built-in**: the loop
  intercepts the model's proposed call, loads the body under the S4 grant gate (ungranted → denied),
  records `SkillActivated` (survives resume), and injects the body. `mcp:skill.activate:call`.

### Part 4 — the ACP stdio adapter (capstone)
- New role crate **`role/acp`** (lib + `lb-acp` bin). Pure `RunEvent <-> ACP` encoder (`encode.rs`)
  + lifecycle driver (`session.rs`) + stdio JSON-RPC loop (`stdio.rs`). Maps `initialize` /
  `session/new` / `session/prompt` / streamed `session/update` / `session/cancel` /
  `session/load`·`session/resume` + a pinned `StopReason`.
- **Trusted-session auth**: the adapter verifies a real `lb_auth` token with the node key, bound to
  one workspace — never a bypass. A token signed by another key is rejected (tested).
- **Disconnect-mid-permission contract** implemented exactly: an Ask → the prompt turn ends with the
  "suspended" StopReason; the pause is durable (the `agent_decision` record), not the connection;
  settled out-of-band via `agent.decide`; the editor picks it back up via `session/resume`. Proven
  end-to-end (`acp_driver_test.rs::disconnect_mid_permission_suspends_durably_and_resumes_out_of_band`).
- Client-provided `mcpServers`/`cwd` rejected cleanly with an ACP error code (not silently dropped).

## Tests (real infra, only the LLM provider stubbed — rule 9)
- Part 0: `crates/jobs/tests/resume_test.rs` (6), `crates/store/tests/create_test.rs` (3 — first-settle),
  `crates/host/tests/agent_rehydrate_test.rs` (3 — rehydration continues; cancel terminal).
- Part 1: `crates/run-events/tests/projection_test.rs` (4 — live==replay).
- Part 2: `crates/host/tests/agent_decision_test.rs` (7 — **first-settle**, deny, isolation, offline/sync)
  + 10 policy unit tests.
- Part 3: `crates/host/tests/agent_watch_test.rs` (4 — snapshot, live deltas, deny, ws-isolation).
- Part 4: `role/acp/tests/acp_stdio_test.rs` (2 — REAL spawned binary over REAL stdio; mcpServers reject),
  `role/acp/tests/acp_driver_test.rs` (3 — auth-deny, disconnect-mid-permission e2e, cancel).
- Part 5: `crates/host/tests/agent_skill_test.rs` (5 — activate, survives resume, grant-deny, ws-iso, catalog).

`cargo test --workspace` → **208 passed, 1 failed**. The 1 failure is
`cross_node_routing_test::a_call_on_the_edge_routes_to_the_extension_on_the_hub` — a **pre-existing**
cross-node Zenoh routing flake, confirmed failing identically on a clean `HEAD` worktree (it touches
none of the agent-run code). `cargo fmt` clean; `cargo build --workspace` clean (0 warnings in the
agent-run crates).

## Decisions made (beyond the scope's resolved set)
- **Cursor is per-transcript-event, turn count is derived** from `AssistantTurn` events — so the
  gateway idempotency key stays keyed by *turn* (replay-safe) while the transcript stores several
  events per turn. (run.rs `count_turns`.)
- **`agent.watch` is an SSE-only verb**, not a JSON-returning MCP verb (a stream has no single value);
  the cap `mcp:agent.watch:call` is checked inside `watch_run` like `bus.watch`. Left an explicit
  `NotFound` arm + comment in `agent/tool.rs`.
- **`skill.activate` is loop-internal** (intercepted, not dispatched out) so the run-state mutation
  (transcript append + context inject) lives where the transcript/cursor/messages are.
- **ACP dev token is minted by the binary with its own key** and verified with the same key — the
  verify path is the real trusted-session check; in dev the binary stands in for the IdP.

## Follow-ups (noted, not built — consistent with scope deferrals)
- A standalone `react_to_decisions` durable-scan reactor (mirroring `react_to_approvals`) to auto-wake
  a settled suspension; v1 resumes via an explicit `resume()`/`session/resume` (tested path).
- `UseDecisionAsResult` resume mode (enum field exists, designed-for); token-level deltas behind a
  gateway streaming `turn`; per-run policy overrides; AI-SDK encoder; catalog re-inject-on-change.
- Wire `lb-acp` into the `node` binary's role registry (it builds + runs standalone today).
- A UI surface for the `/runs/{job}/stream` feed (the browser watcher) — backend is ready.

## Cross-links
- Scope: ../../scope/agent-run/agent-run-scope.md · Public: ../../public/agent-run/agent-run.md
- Part-2 detail: ./part2-policy-decision-session.md
- Debugging: ../../debugging/agent/resume-re-derived-from-goal-not-transcript.md
