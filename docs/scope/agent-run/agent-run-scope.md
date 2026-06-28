# Agent-run scope — a streamable, externally-drivable, interactively-gated run

Status: **shipped** (2026-06-28) — Parts 0–5 built + tested green. See
`public/agent-run/agent-run.md` (the shipped truth) and `sessions/agent-run/agent-run-session.md`
(the working log). All "Resolved decisions" below were followed; the open-question deferrals
(AI-SDK encoder, token deltas, `UseDecisionAsResult`, per-run overrides, decision-reactor, bridging
client MCP servers) remain as the noted next steps.

> **Peer-reviewed 2026-06-28 (codex).** The review confirmed the direction (MCP internal, one
> `RunEvent` vocabulary, ACP as an edge adapter, no Awaken plugin framework) but found that **ACP
> cannot be built on the current `run_session` shape** — the durable run state is not yet real enough
> to replay/resume/suspend. The findings are folded in below: a new **Part 0 (durable run state)**
> prerequisite, corrected **Ask settle semantics** (the existing `Resolution` is last-writer-wins, not
> first-settle — see Part 3), an explicit **ACP disconnect-mid-permission** contract, an
> **event-sourced transcript** stance, and the **start/resume vs watch** split so ACP is not driven by
> a blocking final-answer call. The build order in "Intent" reflects the reviewer's ordering.

The S5 agent (`scope/agent/agent-scope.md`) owns a sound **tool-call loop**, but that loop is a
black box to the outside: `run_session` runs to completion and returns one final `String`
(`rust/crates/host/src/agent/run.rs`), the routed invoke replies with that one string
(`agent/serve.rs`), and the only "live" view is whatever ad-hoc motion a caller posts to a channel.
There is **no typed event stream**, **no external protocol surface** (a `grep` for `acp`/`ai-sdk`/
`ag-ui` across the repo returns nothing), **no mid-run human gate** (approval is coarse — a whole
*job* is gated before it starts, never a single tool call), and **the model cannot pick a skill** —
skills are pre-loaded by the caller, never selected by the model from a catalog.

Worse, the run state is **not durable enough to resume**: on resume `run_session` rebuilds the
message list from only the goal and starts `prior` empty (`run.rs:68`), and a job step is an **opaque
`String`** (`rust/crates/jobs/src/model.rs:33`), not the assistant message / proposed calls / tool
args / tool results / active skills you'd need to faithfully continue a conversation. So "resume at
the cursor" today re-asks the model from scratch — fine for a stateless answer, **not** fine once a
run can suspend mid-conversation for a human decision.

This scope makes the run a **first-class, observable, interruptible, *replayable* object**. Five
parts, in dependency order — Part 0 is the prerequisite the peer review surfaced; the rest build on it:

0. **Durable, typed run state** — a real transcript (messages, proposed calls + args, tool results,
   active skills, pending-suspension id, cursor) that can faithfully **rehydrate** on resume. Today's
   opaque-string step cannot. *Nothing else in this scope is safe without this.*
1. **A canonical run-event stream** — one typed `RunEvent` vocabulary, **derived from** the durable
   transcript (event-sourced), so live streaming and reconnect/replay never diverge.
2. **Per-tool-call Allow / Deny / Ask** — a fine-grained gate that can **suspend** a run for a human,
   durably (so the connection need not be held), with **first-settle** decision semantics.
3. **`agent.watch` + a start/resume split** — observe a run live over SSE; internally separate
   "start/resume a run" from "watch a run" so a lifecycle client (ACP) isn't driven by a blocking
   final-answer call.
4. **An ACP stdio adapter** — let Zed / Cursor / any Agent-Client-Protocol host drive the agent, with
   explicit `session/load`·`session/resume`·`session/cancel` and disconnect-mid-permission handling.
5. **Model-activated skills** — inject a granted-skills catalog; let the model activate one on demand
   (last, because an activated skill must itself survive resume — it lives in Part 0's run state).

The unifying idea (read from the Awaken framework, reviewed 2026-06-28 — ideas taken, code not): the
loop emits **one internal event vocabulary** derived from a durable transcript; every external wire
format is a thin **encoder** over it, and every interactive pause is a **durable suspension**, not a
held connection.

## Goals

- **A durable, typed run transcript** (Part 0): the job step stops being an opaque `String` and
  becomes a structured, append-addressed record of the conversation — assistant turns, proposed tool
  calls with their args, tool results, skill activations, and any pending suspension — sufficient to
  **rehydrate the exact loop state** on resume (today it cannot; `run.rs` re-derives from the goal).
- A `RunEvent` enum (text/reasoning deltas, tool-call start/result, step boundaries, skill
  activation, suspension, done/error) **derived from that transcript** — the single internal contract
  every protocol and UI reads. The **transcript is the durable record**; events are motion (§3 rule
  3), and a reconnecting watcher reconstructs state from the transcript, never from replayed deltas.
- A per-(workspace) **permission policy** (Allow / Deny / Ask, matched on tool name + args) the loop
  consults before each dispatch. **Ask suspends the run durably** and **resumes idempotently** when a
  human decides — surviving the caller disconnecting or the node restarting. The decision uses
  **first-settle** semantics keyed by `{job_id, tool_call_id}` (the existing `Resolution` is
  last-writer-wins, so plain `inbox.resolve` is *not* reused as-is — see Part 3).
- A `agent.watch` MCP tool + gateway SSE route so the browser UI sees a run live (deltas, tool calls,
  skill activation) instead of only a final answer — built on an internal **start/resume vs watch**
  split so a lifecycle client isn't forced through a blocking final-answer call.
- A **thin** ACP stdio adapter (`role/acp`, a new role binary beside `role/gateway`) translating
  JSON-RPC ⇄ `RunEvent`, implementing the ACP v1 turn lifecycle (`session/new`·`prompt`·`update`·
  `request_permission`·`cancel` + `StopReason`) and the persisted-session verbs
  (`session/load`·`session/resume`) that Part 0's durable state makes possible.
- The model **activates a skill from a catalog**: the loop injects the workspace's *granted* skills
  as a catalog before inference; the model calls a `skill.activate` tool to load one on demand. The
  S4 grant still gates the set — the model only self-selects *within* what was granted, and the
  activation is recorded in the run transcript so it survives resume.

## Non-goals

- **AG-UI, A2A, and AI-SDK adapters.** ACP first (editor integration is the highest-value, lowest-
  surface client). AI-SDK v6 (the web `useChat` surface) is the recommended *next* encoder once the
  `RunEvent` stream exists — explicitly deferred here, not designed. AG-UI / A2A are out of scope.
- **A second extension mechanism.** Awaken's in-process `trait Plugin` loop-hook registry is
  **explicitly rejected** — see Intent. We take its *event vocabulary*, *suspension contract*, and
  *skill-catalog pattern*, not its plugin framework. Permission/skills here are host concerns wired
  to existing seams, not a parallel plugin runtime competing with wasm/native + MCP.
- **A new skill store or grant model.** Skills, their assets, and the grant gate are S4
  (`scope/skills/skills-scope.md`); this only adds *catalog rendering + model activation* on top.
- **Replacing MCP.** MCP stays the universal internal contract (rule 7). ACP/AI-SDK are **edge
  translations** that drive the run via the start/resume + `agent.watch` path (with `agent.invoke` kept
  as a compatibility wrapper) and encode the `RunEvent` stream back out.
- **A new model-streaming contract with the gateway.** Whether `ModelAccess::turn` grows a streaming
  variant is an ai-gateway question; this scope ships per-step events now and adds token deltas behind
  a gateway feature-detect later, with no `RunEvent` change (Resolved decisions).
- **Bridging ACP client-provided MCP servers (review point 6).** ACP `session/new` may carry
  `mcpServers` + `cwd` so the agent connects to *client-side* tools. v1 only exposes our already-known
  internal MCP tools; client-provided servers are **explicitly out of scope** (bridging an external
  server the agent calls would need a `net:*`-style grant — Resolved decisions). Stated as a limitation
  so the adapter rejects/ignores them cleanly rather than silently dropping the field.

## Intent / approach

**Build order (peer-review ordering — do not skip ahead).** ACP must not be coded on top of today's
`run_session`. The order is: **Part 0** (durable typed run state + real resume rehydration +
cancellation hook) → **Part 1** (`RunEvent` derived from that state) → **Part 3** (`agent.watch`/SSE
over the vocabulary, plus the start/resume-vs-watch split) → **Part 2** (per-tool policy + durable Ask
with first-settle) → **Part 4** (ACP, once start/resume/cancel/permission semantics exist to map onto)
→ **Part 5** (model-activated skills, which depend on Part 0 so an activated skill survives resume).
The numbering above is by *concept*; this is the *implementation sequence*.

**Part 0 — make the run replayable before anything observes or interrupts it.** Today a job step is an
opaque `String` (`jobs/src/model.rs:33`) and resume re-derives the message list from the goal alone
(`run.rs:68`). That is fine for a one-shot answer and *unsafe* the moment a run can pause mid-
conversation. Part 0 replaces the opaque step with a **typed, append-addressed transcript** carrying
each assistant turn, each proposed tool call **with its args**, each tool result, each skill
activation, and any **pending-suspension id** — enough to **rehydrate the exact loop state**
(messages + `prior` + active skills + cursor) on resume. A `cancel` hook lands here too (a run must be
stoppable; ACP `session/cancel` and a UI stop button both need it). This is the load-bearing slice;
everything else is unsafe without it.

**One vocabulary, derived from the transcript.** `RunEvent` is a small enum in a low-level crate (no
deps on protocols, the gateway, or wasm) — the symmetric dual of how `caps` is the one scope model
projected onto store + bus + MCP. Crucially it is **derived from the durable transcript, not emitted
beside it**: the canonical record is the Part-0 transcript, and both the live stream and a
reconnect/`session/load` replay are *projections* of it (event-sourced). This is the fix for review
point 5 — if the live stream were a separate side-channel, reconnect and live would drift. Each
external protocol is then a pure function `RunEvent -> wire` in its own role crate; the gateway SSE
route is one encoder, the ACP adapter another. Nothing in the loop knows the word "ACP." (Awaken's
`AgentEvent` + per-protocol encoders is the shape we read here; the enum itself we re-derive against
our transcript.)

**Rejected: Awaken's `trait Plugin` loop-hook framework.** Awaken composes the loop from in-process
Rust plugins (phase hooks, tool-gate hooks, tool-policy hooks) registered into the runtime. Adopting
it would stand up a *second* extension model beside our wasm/native + MCP one — directly against rule
1 (symmetric, no parallel paths) and rule 7 (MCP is the contract). We instead add the **specific hook
point we need** (a tool-gate before dispatch, for Ask) directly into the existing loop, and express
permission policy + skill catalog as host services wired to the seams we already have (`caps`,
`lb_inbox`, the S4 skill verbs). Same capability, no new framework.

**The ACP adapter is a role, not a kernel change — but it is not "thin" until Parts 0–3 exist.** It is
a binary like `role/gateway`: it authenticates a local session (reusing the gateway's *trusted-session*
credential path — `role/gateway/src/session/trusted.rs` — never a new auth bypass) and translates the
**ACP v1 turn lifecycle** onto our run primitives. The review corrected an over-simplification here:
ACP is not just `prompt → final string`. It expects `session/new`, `session/prompt`, streamed
`session/update`s, `session/request_permission`, `session/cancel`, a terminal `StopReason`, **and**
persisted-session `session/load` / `session/resume`. The adapter therefore maps onto the **start/resume
vs watch split** (review point 4), not onto a blocking final-answer call:
- `session/new` → start a run (a durable job); `session/prompt` → a turn against it.
- streamed `session/update` ← the `RunEvent` projection (Part 1).
- `session/request_permission` ← a `Suspended` event (Part 2).
- `session/cancel` → the Part-0 cancel hook.
- `session/load` / `session/resume` ← rehydrate from the Part-0 transcript.

**Disconnect mid-permission (review point 3) — an explicit contract, not hand-waving.** ACP
`session/request_permission` is a JSON-RPC *request* the editor is expected to answer, but our Ask is a
**durable suspension** that must outlive the connection. The contract: when a run hits Ask, the adapter
(a) writes the durable decision record, (b) sends `session/request_permission`, and (c) if the editor
answers in-band, settles the decision and resumes immediately; **but if the editor disconnects before
answering**, the run stays suspended and the *current* prompt turn ends with a terminal
`StopReason` we map to "suspended/awaiting-permission" (an ACP `StopReason` such as a refusal/cancel
variant — pinned during build). The decision can then be settled out-of-band (the UI, a reviewer) and
the editor picks the run back up via `session/resume`. The connection is never the thing holding the
pause.

**Ask is a durable suspension with first-settle semantics — NOT plain `inbox.resolve`.** The earlier
draft said "reuse `lb_inbox::Resolution`, re-resolving is a no-op." The review caught that this is
**false against the current code**: `Resolution` is deliberately **last-writer-wins** (it upserts the
same row so a deferred item can later flip to approved — `inbox/src/resolution.rs:30`). For an agent
Ask that is a correctness gap: two reviewers (or a reactor re-scan) could flip an already-acted
decision after the tool already ran. So the Ask decision gets **first-settle** semantics, keyed by
`{job_id, tool_call_id}`:
- Recommended shape: a **dedicated agent-decision record** (`agent_decision:{job}:{tool_call}`) written
  with a **conditional first-write** (create-if-absent; a second decision is rejected, not upserted),
  rather than overloading `Resolution`. It still *surfaces* as an inbox `needs:approval` item for
  routing/visibility, but the settle is on the agent-decision record, not the last-writer-wins
  resolution row.
- Resume is then idempotent on the settled decision + the cursor: a reactor re-scan or a duplicate
  resolve is a no-op because the decision is already settled and the cursor already advanced past it.
This reuses the *S6 reactor pattern* (`react_to_approvals`) for the wake, but **not** the
last-writer-wins resolution write.

**Resume modes.** A resolved Ask can re-enter three ways (Awaken's `ToolCallResumeMode`); we ship the
two that matter and defer the third: **Deny** (feed the model a "denied by policy" tool result — the
loop already handles tool errors gracefully, `run.rs`) and **Allow→replay** (run the originally-
proposed call from the persisted args in the Part-0 transcript). `UseDecisionAsResult` (let the human
hand-write the tool result) is deferred.

**Skills: grant gates the set, the model picks within it.** Today the caller pre-declares a skill on
`agent.invoke` and the loop loads it (`agent/substrate.rs`). The richer pattern: before each
inference the loop renders a catalog of the workspace's *granted* skills (title + description only)
into context; the model calls `skill.activate {id}` to pull the full skill body/assets mid-run. The
grant is still the wall (a skill not granted to the workspace never appears in the catalog and
`skill.activate` denies it). This is purely additive over S4 — it changes *who chooses* (model, not
caller), never *what's allowed* (the grant).

## How it fits the core

- **Tenancy / isolation:** every `RunEvent` stream, permission decision, and skill catalog is
  workspace-scoped. A `agent.watch` in ws-B cannot observe a run in ws-A; a resolution in ws-B cannot
  settle a suspension in ws-A; the ACP adapter's session is bound to exactly one workspace (from its
  token). Proven across store + MCP (mandatory isolation test).
- **Capabilities:** `mcp:agent.watch:call` gates the live stream. The **per-call gate is the same
  `caps::check` chokepoint** the loop already runs (`lb_mcp::call` in `run.rs`) — the permission
  policy is an *additional* Allow/Deny/Ask layer *in front of* it, never a replacement (defense in
  depth: a policy `Allow` still hits `caps::check`). Settling an Ask requires its own
  capability (`mcp:agent.decide:call`) — *who may approve* a tool call is the same authority as
  resolving the surfaced inbox item, routed to a team, but the settle is a first-write on the
  agent-decision record (not the last-writer-wins `inbox.resolve` — see Part 3). `skill.activate` is
  gated by the S4 skill grant. The ACP adapter call carries the session principal — exactly as denied
  as a forged call. Every grant has a deny test.
- **Placement:** *either*, by config. The event stream and suspend/resume are placement-free. The ACP
  adapter is a role binary that runs wherever it is configured — typically co-located with the agent-
  hosting node (hub or solo edge). No `if cloud {…}`.
- **MCP surface** (§6.1 — judged, not defaulted):
  - **Live feed (the core add):** `agent.watch {job_id}` → the `RunEvent` stream, surfaced over the
    gateway SSE route (mirrors `channel_stream` in `routes/stream.rs`). This is motion — a `watch`,
    not a polled `list` (§3.3).
  - **Start / resume vs watch (the split, review point 4):** internally separate "start or resume a
    run" from "watch a run." `agent.invoke` is kept as a **compatibility wrapper** (start + block for
    the final answer) for plain MCP callers, but the lifecycle path (ACP, the UI) uses
    start/resume + `agent.watch` so it is not driven by a blocking final-answer call.
  - **Live feed (the core add):** `agent.watch {job_id}` → the `RunEvent` stream, surfaced over the
    gateway SSE route (mirrors `channel_stream` in `routes/stream.rs`). Motion — a `watch`, not a
    polled `list` (§3.3). A late watcher first gets a snapshot rebuilt from the transcript, then deltas.
  - **Decision:** a **new** `agent.decide {job_id, tool_call_id, decision}` verb with **first-settle**
    semantics — *not* `inbox.resolve`, which is last-writer-wins (Part 3). The Ask still surfaces as an
    inbox `needs:approval` item for routing/visibility, but the binding settle is on the agent-decision
    record.
  - **Skills:** `skill.activate {id}` (read-shaped: loads a granted asset into the run, recorded in the
    transcript). Catalog rendering is internal to the loop, not a separate verb.
  - **Batch:** N/A — a run is inherently a long operation and is *already* a job; there is no bulk
    surface here.
- **Data (SurrealDB):** no new persistence *layer*, but real new run state. The run's durable truth is
  the `job` record with a **typed transcript** (Part 0 — replacing the opaque-`String` step in
  `jobs/src/model.rs`: assistant turns, proposed calls + args, tool results, skill activations, pending-
  suspension id, cursor). The Ask decision is a **dedicated first-settle record**
  (`agent_decision:{job}:{tool_call}`), surfaced as an inbox `needs:approval` item for routing — *not*
  the last-writer-wins `Resolution` row. The **permission policy** is one ws-scoped record
  (`agent_policy:{ws}` — a rule list); the **skill catalog** is derived from the existing S4 skill
  grants (no new table).
- **Bus (Zenoh):** the `RunEvent` stream is motion on a per-run subject (e.g. `ws/{ws}/run/{job}/**`),
  fire-and-forget — a dropped subscriber misses deltas but can re-read the durable job transcript to
  catch up. The stream is **never** the record (§3.3). The routed `agent.invoke` reply is unchanged
  (it still returns the final answer; a remote caller that wants live events subscribes to the run
  subject, which routes like any other bus traffic).
- **Sync / authority:** the job is authoritative on its hosting node. A suspension outlives any
  connection (durable inbox item); the resolution may arrive from a different node/session and is
  applied idempotently on reconnect — the offline/sync mandatory category, reusing the S6
  resolution-reactor path.
- **Secrets:** none new. The ACP adapter authenticates with a session token via the existing trusted-
  session path; it never holds provider keys (those stay with the gateway, §6.7).

## Example flow

A developer wires their editor (an ACP host, e.g. Zed) to the workspace.

1. The editor launches the **ACP adapter** (`role/acp`), which authenticates a trusted local session
   bound to workspace `acme` (reusing `session/trusted.rs`). `session/initialize` handshakes
   capabilities.
2. The developer types "fix the token-refresh race in #2451." `session/new` starts a durable run (a
   job); `session/prompt` drives a turn (the start/resume-vs-watch split — not a blocking
   final-answer call).
3. The loop runs, appending each assistant turn / proposed call / tool result to the **durable typed
   transcript** (Part 0). `RunEvent`s — `TextDelta`, `StepStart`, `ToolCallStart` — are *projected*
   from that transcript; the adapter encodes them as ACP `session/update`s and the editor renders them
   live. The same projection feeds the browser's `agent.watch` SSE if anyone has it open.
4. The model decides it needs the repo conventions. It sees `repo-conventions` in the injected
   **skill catalog** (granted to `acme`) and calls `skill.activate {id: "repo-conventions"}`; the
   loop loads it (S4 grant checked), records the activation **in the transcript** (so it survives
   resume), emits `SkillActivated`, and the body enters context.
5. The model proposes `shell.run {cmd: "rm -rf node_modules"}`. The **permission policy** for `acme`
   matches `shell.run` → **Ask**. The loop **suspends**: it writes the `agent_decision` record
   (pending, keyed by `{job, tool_call}`), surfaces an inbox `needs:approval` item for routing,
   persists the pending-suspension id + cursor in the transcript, emits `Suspended`, and ends the
   turn. The adapter sends ACP `session/request_permission`; the editor shows Allow / Deny.
6. The developer closes the laptop **before answering**. The connection drops mid-permission, so the
   prompt turn ends with the terminal "suspended/awaiting-permission" `StopReason`. **The suspension
   is durable** — it lives in the `agent_decision` record + transcript, never in the connection.
7. Later, a reviewer settles it via `agent.decide … Deny` — a **first-settle** write (a second decide
   is rejected, not upserted). The S6-style reactor wakes the job, **rehydrates the loop state from the
   transcript**, resumes past the settled call, and feeds the model a "denied by policy" tool result.
   The model picks a safer path. A duplicate `agent.decide` or a reactor re-scan is a no-op.
8. The editor reconnects and issues `session/resume`; the adapter replays the transcript projection to
   restore the editor's view, then continues live. The run finishes; `RunFinish` is emitted; the
   durable transcript is the record regardless of who was attached.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`) — the gate, not extras. **No mocks**: tested
against the real store, bus, and a **real spawned ACP adapter** speaking real JSON-RPC over a real
stdio pipe (the pattern of `pnpm test:gateway`, which spawns a real node — rule 9). The **only**
permitted fake is the LLM provider behind `ModelAccess` (a true external, already stubbed at the test
boundary, ai-gateway scope §3).

- **Part 0 — durable run state (the prerequisite, tested first):** a run that performs N steps,
  activates a skill, and is then **reloaded from the store** rehydrates the *identical* loop state
  (messages, `prior` tool results, active skills, cursor) — i.e. a resumed run continues the
  conversation, it does not re-ask from the goal (the current `run.rs:68` behavior is the failing
  baseline this fixes). Cancel mid-run leaves a terminal, restorable state.
- **Capability-deny** (§2.1): `agent.watch` denied without `mcp:agent.watch:call`; a principal lacking
  `mcp:agent.decide:call` cannot settle an Ask; `skill.activate` denied for a skill not granted to the
  workspace; the ACP adapter call denied without its session grant.
- **First-settle semantics (review point 2):** two `agent.decide` calls on the same
  `{job, tool_call}` → the **first** binds, the second is **rejected** (not a silent upsert); a decide
  arriving *after* the tool already ran/was denied is a no-op. This is the test that would FAIL against
  plain `lb_inbox::Resolution` (last-writer-wins) — it proves the dedicated record.
- **Workspace-isolation** (§2.2): ws-B cannot `agent.watch` a ws-A run; a ws-B `agent.decide` cannot
  settle a ws-A suspension; a ws-B catalog never lists a ws-A-only skill — across store + MCP.
- **Offline / sync** (§2.3): suspend → **drop the ACP connection mid-permission** / restart the node →
  the suspension survives (the `agent_decision` record + transcript, not the connection) → a later
  `agent.decide` rehydrates and resumes the run exactly once (duplicate decide and reactor re-scan do
  not double-apply or re-spend the gateway budget — the per-step idempotency key, `run.rs`).
- **Unit:** the `RunEvent` projection from the transcript (live deltas and a `session/load` replay
  yield the same view — review point 5); the encoders (`RunEvent → ACP session/update`,
  `RunEvent → SSE Event`); the policy evaluator (Allow/Deny/Ask glob on tool name + args; Deny beats
  Allow beats Ask); resume-mode application (Deny → error result; Allow → replay from persisted args);
  the suspend point persists the pending-suspension id + cursor *before* emitting `Suspended`; injected
  clock/ids (determinism, no wall-clock).
- **Integration:** a real spawned ACP adapter driving a full `session/new` → `prompt` → live
  `session/update`s → `request_permission` → **disconnect** → terminal `StopReason` →
  `agent.decide` out-of-band → `session/resume` → `RunFinish`; a real-gateway SSE test of `agent.watch`
  showing a transcript-snapshot-then-deltas late join + a `SkillActivated` event (extends the
  `*.gateway.test.tsx` suite).

## Risks & hard problems

- **Durable run state is the real prerequisite (the biggest risk).** Per the peer review, the current
  opaque-`String` step + goal-only resume cannot faithfully replay a suspended conversation. If Part 0
  is skipped or half-built, every downstream part (resume, Ask, ACP `session/load`, surviving skills)
  is silently wrong. This is sequenced first and gated by its own test for that reason.
- **The record is the transcript; the stream is a projection.** Don't treat the `RunEvent` feed as
  truth, *and* don't emit it as a side-channel beside the transcript (it would drift from
  `session/load`/reconnect — review point 5). The transcript is the single source; live and replay are
  both projections of it (§3.3, ai-gateway "partial streams are never the record").
- **First-settle vs the existing last-writer-wins `Resolution` (review point 2).** `lb_inbox::Resolution`
  upserts (last-writer-wins, by design — `inbox/src/resolution.rs:30`). An agent Ask needs the opposite:
  once a tool call is decided and acted on, a later decision must not flip it. Hence the dedicated
  `agent_decision` record with a conditional first-write, *not* a reuse of `inbox.resolve`. Getting this
  wrong = a tool runs (or is denied) and then the decision changes underneath it.
- **ACP disconnect mid-permission (review point 3).** `session/request_permission` is a JSON-RPC request,
  but our pause is durable. The contract (Intent) must be implemented exactly: connection drop → terminal
  "suspended" `StopReason`, decision settled out-of-band, run picked up via `session/resume`. The failure
  mode if hand-waved: a dropped editor wedges a run waiting on a reply that never comes.
- **ACP is not a one-call wrapper (review point 4).** Driving ACP through a blocking `agent.invoke`
  final-answer call cannot express updates/cancel/permission/stop-reason. The start/resume-vs-watch
  split must exist before the adapter; `agent.invoke` stays only as a compatibility wrapper.
- **Protocol drift.** ACP (and later AI-SDK) are external specs that move; ACP also defines
  `session/load`·`session/resume` and **client-provided MCP servers** (`mcpServers` + `cwd` on
  `session/new`) we do *not* bridge in v1 (see Non-goals). Containment: the `RunEvent` enum + the
  transcript are the stable internal contract; each protocol is a thin, version-pinned encoder in its
  own role crate.
- **Authn for a local stdio adapter.** Tempting to let a local process skip auth. Don't — the adapter
  must carry a real session token (trusted-session path) bound to one workspace, or the workspace wall
  has a hole at the editor. Reuse `session/trusted.rs`, never a bypass.
- **Policy surface creep.** Allow/Deny/Ask matched on *args* invites a mini query language. Start with
  glob on tool name + a shallow arg-path match; resist regex/JSONPath until a real caller needs it.
- **Token-delta events depend on the gateway.** If `ModelAccess::turn` stays non-streaming, the run
  emits per-step events, not per-token deltas — still a large UX win, but set expectations. Token
  deltas are gated on the gateway streaming (decided below: per-step v1, token deltas behind the
  feature-detect when the gateway grows a streaming turn).

## Resolved decisions

The peer review's open questions are **resolved here** — these are the long-term answers the build must
follow (no quick fixes). They are decisions, not options.

- **Transcript shape + location → typed events inline on the `job`, behind a versioned enum.** Replace
  `Step.result: String` with `Step` carrying a `TranscriptEvent` (a `#[non_exhaustive]`,
  `#[serde(tag = "kind")]` enum: `AssistantTurn`, `ToolCallProposed{id,name,args}`, `ToolResult{id,ok,
  err}`, `SkillActivated{id}`, `SuspensionOpened{tool_call_id,decision_id}`, `SuspensionSettled{
  decision_id,decision}`). Inline on the job record (runs are `MAX_STEPS`-bounded). **Long-term guard:**
  the enum is versioned from day one (`#[non_exhaustive]` + a `schema_version` field on the job) so a
  later child-table migration or new variant is additive, never a rewrite. Rationale: one record =
  one atomic resume read; a child table buys nothing at bounded run length and costs a join + a second
  write path. The escape hatch is designed-in, not the v1 cost.
- **Ask settle → a dedicated first-settle `agent_decision` record.** `agent_decision:{job}:{tool_call}`
  written with a **conditional create** (SurrealDB `CREATE`, which errors on an existing id — first write
  binds, second is rejected, never an upsert). It surfaces an inbox `needs:approval` item for routing
  and visibility, but the binding settle is the `agent_decision` record. **Do not** add a settle-mode to
  `lb_inbox::Resolution` — the coding workflow depends on its last-writer-wins semantics; overloading it
  couples two consumers with opposite requirements. This is the long-term-correct separation.
- **Permission policy → a standalone ws-scoped record (`agent_policy:{ws}`) edited by an admin cap.**
  Not folded into the capability grammar: Allow/Deny/Ask is a *runtime policy over tool calls*, not a
  static capability, and conflating them would distort `caps`. One record per workspace, a rule list
  (glob on tool name + a shallow arg-path equality match — **no** regex/JSONPath in v1, the surface is
  deliberately small), edited via `agent.policy.set` gated by an admin-only cap. The policy layer sits
  *in front of* `caps::check`, never replacing it (defense in depth).
- **Resume modes → ship `Deny` + `Allow→replay`; `UseDecisionAsResult` is designed-for but not built.**
  The resume-mode is an enum field on the `agent_decision` record so adding the human-authored-result
  mode later is additive. v1 builds the two that the policy gate actually produces.
- **ACP client-provided MCP servers (`mcpServers`/`cwd`) → explicitly rejected in v1, cleanly.** The
  adapter parses the field and returns an ACP error (not a silent drop) stating client-provided servers
  are unsupported. Long-term path noted: bridging them is a *future* scope requiring a `net:*`-style
  grant (reference-extensions scope) — it is not smuggled in now.
- **Stream routing → the `RunEvent` subject routes over the bus like any motion; the start/resume reply
  stays a value, not a stream.** A remote `agent.watch` subscribes to `ws/{ws}/run/{job}/**` over Zenoh
  (the S3 routing seam); we do **not** stream the run over the queryable reply. State-vs-motion holds:
  the reply is a result, the stream is motion, the transcript is the record.
- **Token deltas → per-step events in v1; token-level deltas behind a gateway feature-detect.** The
  `RunEvent` enum includes `TextDelta` from day one, but the loop emits per-step until `ModelAccess`
  grows a streaming `turn`; when it does, the loop forwards token deltas with no `RunEvent` change. The
  enum is designed for the streaming end-state so adding it later touches the gateway, not the contract.
- **Next encoder after ACP → AI-SDK v6 (web `useChat`), not built in this scope but designed-for.** The
  `RunEvent` enum carries explicit tool-call **argument deltas** from day one (AI-SDK needs them) so the
  second encoder is purely additive — a new role crate, no contract change.
- **Skill-catalog injection → render once per run, cache, re-inject only on change.** The granted-skills
  catalog (title+description) is computed at run start and cached on the run; it is re-rendered into
  context only when the granted set changes mid-run (rare). Avoids paying the catalog token cost every
  turn.
- **Per-run policy overrides → not in v1; the policy is ws-scoped only.** A run-scoped "allow `shell.run`
  just this session" override is deliberately deferred. The `agent_policy` evaluation is structured so a
  per-run override layer can be added above the ws record later without changing the gate — additive, not
  a rework.

## Related

- README `§6.16` (shared AI agents — this makes their run observable/interruptible), `§6.13` (the
  SSE/HTTP gateway the `agent.watch` route extends), `§6.5` (MCP — the contract ACP translates to/from),
  `§6.9` (jobs — the durable run record), `§6.10` (inbox/outbox — the suspension + resolution facet).
- `../agent/agent-scope.md` — the S5 loop this opens up (the `RunEvent` stream replaces its `String`
  return; the per-call gate wraps its `lb_mcp::call`).
- `../ai-gateway/ai-gateway-scope.md` — whether model **token** deltas are available (the streaming
  question) and the idempotency-key cache that keeps resume from re-spending.
- `../jobs/jobs-scope.md` — the `job` record whose opaque `Step.result` Part 0 replaces with a typed,
  rehydratable transcript; the cursor the run suspends/resumes against.
- `../inbox-outbox/outbox-scope.md` — the S6 resolution-reactor an Ask reuses for the *wake*; note the
  Ask **settle** is a dedicated first-settle record, NOT the last-writer-wins `Resolution` (Part 3).
- `../skills/skills-scope.md` — the S4 grant gate + skill assets the model-activated catalog sits on.
- `../auth-caps/auth-caps-scope.md` — `caps::check` (the chokepoint the policy layers in front of) and
  the trusted-session authn the ACP adapter reuses.
- `../../vision/0002-coding-agent-workplace.md` — the worked example this directly serves (an editor
  driving the central coding-agent, with a human gate on dangerous tool calls).
## Source review & attribution

This scope was shaped by **reading** the [Awaken](https://github.com/AwakenWorks/awaken) framework
(reviewed 2026-06-28, dual-licensed MIT/Apache-2.0 — same as this repo). **We read it for ideas; we do
not copy its code.** Awaken's abstractions assume its own runtime, so lifting code would fight our
architecture more than help it. Where a snippet ever *is* referenced, the shared MIT/Apache license
permits it **with attribution** — record it in the implementing session and a code comment.

Worth re-opening the source for, when building each part:

| Part | What to read in Awaken | What to take | What to leave |
|---|---|---|---|
| Event stream | `awaken-runtime-contract/src/contract/event.rs` (`AgentEvent`, ~14 variants) | The *shape*: one internal vocabulary, protocols are encoders over it. Re-derive a smaller enum against **our** loop states. | The enum verbatim — ours maps to our `CallOutcome`/cursor/`ModelAccess`, not theirs. |
| ACP adapter | `awaken-server/src/protocols/acp/stdio.rs` + `acp/encoder.rs` (<600 LOC) | A worked reference for "how thin can the adapter be." The **ACP wire spec itself** ([agentclientprotocol.com](https://agentclientprotocol.com)) is the real source — external, not Awaken's. | Their server/dispatch wiring (we have a gateway + Zenoh routing already). |
| Per-call Ask | `awaken-runtime-contract/.../suspension.rs` (`SuspendTicket`, `ToolCallResumeMode`) | The **resume-mode contract** (Deny / Allow→replay / use-as-result) as a design pattern. | Their mailbox — we suspend onto `lb_inbox::Resolution` + the job cursor (the S6 reactor). |
| Skills | `awaken-ext-skills/` (catalog discovery plugin + `skill` activation tool) | The pattern: inject a granted-skills catalog, model activates on demand. | The plugin framework — built on our S4 grant gate as host concerns. |
| (rejected) | `awaken-runtime/src/plugins/lifecycle.rs` (`trait Plugin` + hook registry) | — | **The whole framework.** A second extension mechanism vs our wasm/native+MCP (rules 1 & 7). We add only the one tool-gate hook we need, inline. |

The clone reviewed lived at `/tmp/awaken` (not vendored into this repo; re-clone to revisit). The
full architectural map produced during review is summarized in this doc's Intent and the table above —
no separate notes file is kept, to avoid a stale second source of truth.
