# Agent-run (public)

Status: **shipped** (Parts 0–5). The run is now a first-class, observable, interruptible, replayable
object. Scope: `../../scope/agent-run/agent-run-scope.md` · Session:
`../../sessions/agent-run/agent-run-session.md`.

## What you can rely on

### The durable typed transcript (the record)
A job step is a typed `lb_jobs::TranscriptEvent` (`#[non_exhaustive]`, `#[serde(tag="kind")]`):
`AssistantTurn`, `ToolCallProposed{id,name,args}`, `ToolResult{id,ok,err}`, `SkillActivated{id}`,
`SuspensionOpened{tool_call_id,decision_id}`, `SuspensionSettled{decision_id,decision}`. The job
carries a `schema_version` (versioned from day one). `JobStatus` adds `Suspended` (resumable) and
`Cancelled` (terminal). Resume **rehydrates** the exact loop state (messages + prior results + active
skills) from the transcript — it does **not** re-ask from the goal.

### The `RunEvent` vocabulary (motion, derived from the record)
`lb_run_events::RunEvent` (low-level crate, no protocol deps): `RunStart`, `StepStart`, `TextDelta`,
`ReasoningDelta`, `ToolCallStart`, `ToolCallArgsDelta` (AI-SDK-ready), `ToolCallResult`,
`SkillActivated`, `Suspended`, `Settled`, `RunFinish{outcome}`. `project(job)` = a late watcher's
snapshot; `project_one(event, turn)` = the live deltas the loop emits. **Live and a `session/load`
replay are the same projection of the same transcript — they never diverge.**

### Observe a run: `agent.watch` + SSE
- Host: `lb_host::watch_run` → a transcript snapshot then live `RunEvent` deltas, gated by
  `mcp:agent.watch:call`, workspace-walled (subject `ws/{id}/run/{job}/events`).
- Gateway: `GET /runs/{job}/stream?token=` (mirrors the channel stream) — `event: run` frames.
- The **start/resume-vs-watch split**: driving (`invoke`/`resume`) is separate from observing
  (`agent.watch`); `agent.invoke` remains a compatibility wrapper (start + block for the answer).

### Gate a tool call: Allow / Deny / Ask
- Policy: one ws record `agent_policy:{ws}` (glob on tool name + shallow arg-path equality;
  **Deny > Allow > Ask**; default-allow), edited by `agent.policy.set` (admin cap). It sits **in front
  of** `caps::check` (defense in depth).
- Ask **suspends the run durably** with **first-settle** semantics: a dedicated
  `agent_decision:{job}:{tool_call}` record written via `lb_store::create` (first write binds, a
  second is a `Conflict` — NOT the last-writer-wins inbox `Resolution`). Surfaced as a
  `needs:approval` inbox item for routing. Settled by `agent.decide` (`mcp:agent.decide:call`).
  Resume modes: **Deny** (denied-by-policy result) and **Allow→replay** (re-run from persisted args).

### Drive from an editor: the ACP adapter (`role/acp`, `lb-acp` binary)
ACP v1 over stdio: `initialize` / `session/new` / `session/prompt` / streamed `session/update` /
`session/cancel` / `session/load`·`session/resume` + a pinned `StopReason`. Authenticates a real
session token bound to one workspace (the trusted-session path — never a bypass). **Disconnect
mid-permission is durable**: an Ask ends the turn with a "suspended" StopReason, the pause lives in
the `agent_decision` record (not the connection), settles out-of-band, and the editor resumes via
`session/resume`. Client-provided `mcpServers`/`cwd` are rejected with a clean ACP error.

### Model-activated skills
The loop injects the workspace's *granted*-skills catalog (title+description) once per run; the model
calls `skill.activate {id}` (loop-internal; the S4 grant is the wall — an ungranted skill is denied).
The activation is recorded in the transcript, so it survives resume.

## Not in v1 (deferred, designed-for)
AI-SDK/AG-UI/A2A encoders; token-level deltas (behind a streaming gateway `turn`);
`UseDecisionAsResult` resume mode; per-run policy overrides; a standalone decision-reactor; bridging
client-provided MCP servers; a browser UI for the run feed.
