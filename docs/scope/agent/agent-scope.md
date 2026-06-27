# Agent scope — the central, workspace-scoped AI agent

Status: scope (the ask). `public/agent/agent.md` is the stub that gets *filled* when the S5 slice
ships (the file already exists as a placeholder — it is not created on ship, only completed).

A **central AI agent** is a workspace-scoped service actor hosted on the hub and callable by
edge users over the routed MCP namespace (README §6.16). It is *not* a special core mode and
*not* a model: it is an actor that owns the **tool-call loop** — ask the AI-gateway for a
completion, run any proposed MCP tool calls (each capability-checked, workspace-first), feed the
results back, repeat — over a substrate of **granted skills** and **shared docs** (S4). Its
long-running session is a durable, resumable **job** (README §6.9) so it survives the edge
disconnecting. The point of S5 is to prove that an edge user can invoke a hub-hosted agent, the
agent can reach a model and a granted tool, and the session outlives the caller.

## Goals

- A `agent.invoke` MCP tool: an edge user calls the central agent over the **same routed MCP
  namespace** as any other tool (reuse the S3 Zenoh-queryable routing). `caps::check` runs on
  the calling node, workspace-first.
- The agent owns the **tool-call loop**; the AI-gateway does **model access only** (ai-gateway
  scope). The loop is bounded (max iterations + budget) and every tool call inside it re-runs
  `caps::check`.
- The agent's effective capabilities are the **intersection** of what it was delegated and what
  its own actor token holds — an agent can never *widen* its own access (auth-caps delegation).
- The agent loads **granted skills** (S4 `load_skill`) and reads **shared docs** (S4 `get_doc`)
  as substrate — capability- and membership-checked, never bypassing the three gates.
- The session is a durable **job** (README §6.9): progress streams to a channel (motion),
  attention is written to the inbox (state), and the job survives + resumes idempotently after
  the edge disconnects.

## Non-goals (S5)

- A "coding agent" product (S6) — S5 builds the *generic* agent actor + loop, not the worked
  example. The coding workflow (issue → triage → approval → job → outbox) is S6.
- The transactional must-deliver **outbox** relay (§6.10) — S5 streams progress as motion and
  persists session state; the durable outbox with a delivery cursor is the next slice (still
  open in the inbox-outbox scope). S5 external effects are queued as job-owned state, not yet
  relayed through a cursor-driven outbox.
- A real model provider — the provider HTTP is **mocked at the test boundary** (testing §3,
  deterministic, no network). The gateway contract is real; the adapter behind it is a stub.
- Multi-agent orchestration, agent-to-agent delegation chains, agent mentions in channels — all
  later. S5 is one agent, invoked once, running one bounded loop.

## Intent / approach

The agent is **a host service that holds the loop**, sitting beside `channel/` and `assets/` —
not a new crate, not a wasm extension. Why a host service: the loop must call `caps::check`
directly on each tool dispatch, read S4 assets through the existing host verbs, and drive a job
record — all host-internal seams. A wasm extension would have to round-trip every one of those
back through the host anyway.

**The loop is the agent; the gateway is a function.** This is the load-bearing split (ai-gateway
scope): `gateway.complete(AiRequest) -> AiResponse` is stateless model access. The agent reads
the response's proposed `ToolCall`s, runs each through `lb_mcp` (capability-checked,
workspace-first, possibly routed), appends the `ToolResult`s, and calls the gateway again — until
the model returns no tool calls or the agent hits its iteration/budget ceiling. Keeping the loop
out of the gateway is exactly what lets the gateway be a swappable sidecar.

**Delegated caps are an intersection, never a widening.** When a principal invokes the agent, the
agent acts under a derived principal whose caps are `caller.caps ∩ agent.caps` — the agent can do
only what *both* it and the caller may do. This is the S5 piece of the auth-caps "grant
delegation" open question: the grammar already supports subsetting (a cap is held or not); the new
thing is the *issuance* — deriving the narrower principal. An agent token that listed
`store:doc/**:read` cannot read a doc the *caller* couldn't, and vice-versa. Rejected: letting the
agent run under its own (possibly broader) token — that would make "invoke the agent" a privilege
escalation, the exact anti-pattern §6.16 warns against ("agents are workspace-scoped actors, not
global super-users").

**Durable session = a job record, resumed idempotently.** The agent's session state (the running
transcript: messages + tool results + the loop cursor) lives in a `job:{id}` record (jobs scope),
workspace-scoped. The edge invoking the agent does not hold the session — it kicks off the job and
streams progress. If the edge disconnects mid-loop, the job's state is already durable; a `resume`
re-reads the record and continues from the cursor. Re-running a completed step is a no-op (the
transcript is append-addressed by step index), so resume is idempotent — the offline/sync
mandatory category.

## How it fits the core

- **Tenancy / isolation:** the agent is workspace-scoped. Every gateway call, tool call, skill
  load, and doc read carries the workspace and runs `caps::check` (gate 1 = workspace) first. An
  agent invoked in workspace B can never see workspace A's docs/skills/tools — proven across store
  + MCP (mandatory isolation test). The job record carries `ws` (the hard wall, jobs scope).
- **Capabilities:** `mcp:agent.invoke:call` gates invoking the agent. *Inside* the loop, every
  tool dispatch re-runs `caps::check` under the **derived** (intersected) principal — so a granted
  `agent.invoke` never implies the tools the agent may then call. The deny path: an agent asked to
  call a tool neither it nor the caller holds is refused at the same chokepoint (capability-deny
  test), and the model is told the call was denied (the loop continues; denial is not a crash).
- **Placement:** *either*, by config (symmetric nodes). The default is hub-hosted (shared model
  access, heavier compute, survives edge disconnect). The same code runs solo on an edge for
  local/offline operation — the gateway it talks to resolves to a local provider (ai-gateway
  scope). No `if cloud {…}`; placement is which node serves the `agent.invoke` queryable.
- **MCP surface:** consumes — every tool the agent calls is an MCP tool over the routed namespace
  (the universal contract, §3.7). Exposes — `agent.invoke` is itself a host-native MCP tool (like
  `assets.*`), reached through `lb_mcp::authorize_tool` then delegating to the agent service.
- **Data (SurrealDB):** the session is a `job:{id}` record (status, kind, payload, cursor,
  transcript, attempts, `ws`, ts). State, not motion — the transcript is the durable source of
  truth. The substrate (docs, skills) are the S4 records, read through the host verbs.
- **Bus (Zenoh):** two uses. (1) **routing** — `agent.invoke` rides the S3 queryable
  (`mcp/agent/call`) edge→hub. (2) **motion** — progress streams to a channel as ephemeral
  messages (the live "agent is thinking / called tool X" echo); the durable record is the job, not
  the stream (state-vs-motion, §3.3).
- **Sync / authority:** the job is hub-authoritative when hub-hosted. Offline behavior: the edge
  may disconnect after kicking off the job; the hub continues the loop, persisting each step. On
  reconnect the edge reads the job's durable progress (the same `(table,id)` upsert the channel
  sync path covers). Resume is idempotent (append-addressed transcript).
- **Secrets:** the agent never holds provider keys — those live with the **gateway** (§6.7,
  ai-gateway scope), envelope-encrypted, never handed to the caller or the agent. N/A to the agent
  service directly.

## Example flow

1. An edge user calls `agent.invoke` over MCP with `{ goal: "summarize the design doc",
   skill: "summarize", doc: "scope-x" }`. `caps::check` runs on the **edge** (gate 1 workspace,
   gate 2 `mcp:agent.invoke:call`); the call routes over the Zenoh queryable to the hub.
2. The hub's agent service derives the principal: `caps = caller.caps ∩ agent.caps`. It creates a
   durable `job` record (`ws`, kind `agent-session`, the goal as payload, cursor 0).
3. The agent loads the granted skill (`load_skill`, gate 3 = grant) and reads the doc (`get_doc`,
   gate 3 = membership) — both capability- and membership-checked under the derived principal.
4. The agent calls the **gateway**: `AiRequest { messages, tools, budget, idempotency_key }`. The
   gateway (mock provider at the test boundary) returns content and/or proposed tool calls.
5. For each proposed tool call, the agent runs `lb_mcp::call` (capability-checked, workspace-first,
   routed if remote). Results are appended to the transcript; the cursor advances; the job record
   is updated (durable after every step).
6. The agent streams progress to a channel (motion) and writes any attention to the inbox (state).
7. **The edge disconnects.** The hub keeps running the loop — the session is the job, not the
   connection. Each step persists.
8. The edge reconnects and reads the job's progress; or a `resume` re-reads the record and
   continues from the cursor. A re-applied step is a no-op (idempotent). The loop ends when the
   model returns no tool calls or the iteration/budget ceiling is hit; the job is marked `done`.

## Testing plan

Mandatory categories (testing §2) — these are the S5 gate, not extras:

- **Capability-deny** (§2.1):
  - `agent.invoke` denied without `mcp:agent.invoke:call` (the MCP gate, before the loop runs).
  - Inside the loop: the agent cannot call a tool / load a skill / read a doc the **derived
    principal** wasn't granted — workspace-first. A tool the *caller* lacks is denied even if the
    agent's own token lists it (the intersection holds: no widening).
- **Workspace-isolation** (§2.2): an agent invoked in workspace B can never see workspace A's
  docs / skills / tools — across **store + MCP**. The job record is ws-scoped (a ws-B resume can't
  read a ws-A job).
- **Offline / sync** (§2.3): a workflow job survives the edge disconnecting and **resumes
  idempotently** — re-running from the cursor does not double-apply steps or re-spend the gateway
  budget (the gateway caches by idempotency key, ai-gateway scope).
- Unit: the derived-principal intersection (`caller ∩ agent`), the loop terminator (no-tool-calls
  and ceiling), the cursor advance, injected clock/ids (determinism, §3 — no wall-clock).
- Integration (real embedded SurrealDB + in-proc Zenoh; mock only the provider HTTP): the full
  invoke → gateway → tool-call → job-persist path; the routed edge→hub invoke.

## Risks & hard problems

- **The intersection is security-critical.** A bug that widens (union instead of intersection, or
  drops gate 1) is a privilege escalation. The derive step is one small, heavily-tested function;
  the loop calls the *same* `caps::check` chokepoint, so there is no second authorization path to
  get wrong.
- **Resume idempotency under partial failure.** A step that persisted its result but crashed before
  advancing the cursor must not re-spend the gateway budget on resume — hence the gateway's
  idempotency-key cache (ai-gateway scope) and the append-addressed transcript. The contended case
  is "did this step complete?"; addressing the transcript by step index makes the answer a lookup,
  not a guess.
- **Bounded loop.** Without an iteration + budget ceiling an agent can loop forever or burn budget.
  The ceiling is enforced by the agent (the caller of the gateway), not the gateway.
- **Streaming vs durability.** Progress is motion (ephemeral); the record is the job. A partial
  stream is never the source of truth (ai-gateway scope: "partial streams are never the record").

## Open questions

- Per-workspace agent **instance** vs a shared hub **pool** (README §13 open question) — S5 hosts
  one agent per node; the pool/instance policy is deferred. Does the agent token live per-workspace
  or is it a hub actor scoped per-call? (S5: scoped per-call via the derived principal.)
- The **derived principal's `sub`** — does the agent act as `agent:{id}` or as the caller
  (`on-behalf-of`)? S5 uses a distinct `agent:{id}` sub with the intersected caps, so audit shows
  "the agent acted" with the caller recorded in the job payload. Confirm against the audit schema
  (ai-gateway scope open question).
- **Where the loop ceiling is configured** — per-workspace policy vs a fixed default. S5 uses a
  fixed default; the policy projection (§6.6) is a follow-up.
- **Outbox integration** — S5 queues external effects as job-owned state; wiring the cursor-driven
  outbox relay (§6.10) is the inbox-outbox follow-up (and the S6 coding-workflow driver).
- **Agent-as-channel-actor** — mentioning the agent in a channel / assigning it an inbox item
  (§6.16) — deferred to S6 with the worked example.

## Related

- README `§6.16` (shared AI agents & workflow extensions), `§6.15`/`§6.14` (AI gateway),
  `§6.9` (jobs), `§6.10` (inbox/outbox), `§6.5` (MCP), `§7` (tenancy — agents as ws-scoped actors).
- `../ai-gateway/ai-gateway-scope.md` — the model-access contract the agent calls (the gateway
  does *not* own the loop).
- `../jobs/jobs-scope.md` — the durable, resumable session record the agent drives.
- `../auth-caps/auth-caps-scope.md` — grant **delegation** (the intersection): this scope is the
  S5 resolution of that open question.
- `../skills/skills-scope.md`, `../files/files-scope.md` — the granted skill + shared doc substrate
  (S4) the agent reads through the host's three-gate verbs.
- `../../vision/0002-coding-agent-workplace.md` — the S6 worked example this generic agent enables.
