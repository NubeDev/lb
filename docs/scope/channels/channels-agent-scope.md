# Channels scope — in-channel agent (ask an agent, get an answer, in the channel)

Status: **v1 + live run-feed + background execution shipped.** v1 (inline worker; external agent driven
live vs Z.AI GLM-4.6) + the live run-feed — see
[channels-agent-session.md](../../sessions/channels/channels-agent-session.md). **Background/durable
execution** (the run is now a durable enqueue job drained by a background reactor off the POST
connection — survives tab close + node restart, idempotent) — see
[channels-agent-background-session.md](../../sessions/channels/channels-agent-background-session.md).
**Supervision** (wall-time ceiling + kill/reap of a hung run) and external-agent #3/#4 remain follow-ups.
Promotes fully to `public/channels/` once supervision lands.
Topic: `channels`. Builds on `channels-scope.md` (registry/history/stream), `channels-query-charts-scope.md`
(the inline-worker pattern this reuses), the shipped `agent.invoke` / `AgentRuntime` seam
(`scope/external-agent/runtime-seam-scope.md`), and the shipped agent **run** surface
(`scope/agent-run/agent-run-scope.md`: durable job, `RunEvent` stream, `agent.watch`/SSE, per-tool policy).

A channel member asks an agent a question by posting it into a channel. A host-side worker starts a
**durable agent run** under the poster's principal, the run streams its progress (tool calls, partial
output) live into the channel as motion, and its final answer is persisted back as a structured channel
`Item`. The query, who asked, the agent's live work, and the answer all live in one ordered channel
history — turning a channel into a shared, durable, auditable **agent workspace**, the same way
`channels-query-charts` turned it into a query notebook.

The agent behind it is selected through the shipped `AgentRuntime` seam by a `runtime` field — the
**default in-house loop today**, an external ACP agent (Open Interpreter / VT Code) once the
external-agent safety wall (`scope/external-agent/capability-wall-scope.md` #3) ships. The channel side
does not change when the runtime does: that is the whole point of building against the seam, not the
agent.

## Goals

- Ask an agent **in a channel** and get the answer back **in the same channel**, durably.
- Show the agent's work **live** (tool calls, streamed tokens) as it runs — not a frozen spinner — by
  reusing the shipped `agent.watch`/SSE run stream. The final answer is a durable channel `Item`.
- The question, the asker's identity, the run transcript, and the answer are one ordered history the
  whole channel can scroll — the same audit property `query_result` items already give.
- Select the runtime by a `runtime` field on the request (default in-house now; external profile later)
  with **no channel-side code change** when the runtime swaps — build against the `AgentRuntime` seam.
- Reuse the channel transport (history + bus + SSE) and the existing `agent.invoke` gate + agent-run
  job/stream/policy machinery — **no new transport, no new run engine, no new run stream.**
- Access stays capability-first and workspace-scoped: asking an agent needs *both* channel `pub` and the
  `agent.invoke` grant; every tool the agent then calls re-checks its own grant under the derived principal.

## Non-goals

- No new agent loop, model provider, or run engine. `agent.invoke` → `invoke_via_runtime` is the only
  execution path; the run is the existing agent-run durable job.
- No new runtime. This feature *selects* a runtime through the shipped seam; it does not build one. The
  external runtime's safety (`external-agent` #3 wall) is that topic's gate, not this one's — until #3 is
  green, only the always-registered in-house `default` runtime is reachable from a channel.
- No per-tool approval UI in this slice. The agent-run policy (`agent_policy:{ws}`, Deny>Allow>Ask) already
  governs tool calls; an in-channel **Allow/Deny prompt card** is a fast-follow (Open questions), not v1.
- No multi-turn threaded conversation with the agent in this slice — one `agent` item is one run with one
  final answer. Follow-up is a new `agent` item (a new run). Threading is a later scope.
- No new channel-kinds machinery beyond the payload tags this needs (mirrors query's additive tags).
- No agent *config* surface (choosing models, editing personas) — that is the gateway/skills scope.

## Intent / approach

A channel `Item` payload is opaque, so we use it as a typed envelope — exactly as `channels-query-charts`
does. The agent exchange shares the channel via a `kind` field on the payload:

- `kind: "agent"` — `{ goal, runtime?, skill?, job }`, posted by a member who wants to ask an agent.
  `runtime` absent = the default in-house loop; a value = a named runtime/profile (resolved through the
  seam, grant-gated). `job` is the durable run id the UI mints up front so it can subscribe to the live
  stream the instant the request lands (mirrors how `AgentView` passes a `jobId`).
- `kind: "agent_result"` — `{ goal, runtime, job, answer }`, posted by the worker on completion. The
  durable final answer.
- `kind: "agent_error"` — `{ goal, error }`, posted by the worker when the run cannot start or fails.
  Opaque on the deny/no-such-runtime path (no capability/existence leak); honest on an execution fault.
- (absent / `kind: "chat"`) — an ordinary message. Untagged stays chat, so this is purely additive and
  existing channels are unaffected.

The flow is **request → live run → durable response over the channel**, and this is the one deliberate
divergence from the query worker:

- **The query worker runs inline to completion** — a SELECT is bounded and fast, so `post` blocks on it
  and returns the `query_result`. An **agent run is long** (many tool calls, model latency), so blocking
  `post` on it is wrong. The agent worker instead **spawns the run as a durable agent-run job and returns
  immediately** — the post succeeds the moment the `agent` request item lands. The run then drives itself
  under the job, and the `agent_result` item is posted asynchronously when the run completes.
- **"Both" outputs, via state-vs-motion (§3.3), no new infra:**
  - *Motion (live):* the run emits `RunEvent`s over the shipped `agent.watch` subject
    (`ws/{id}/run/{job}/events`, gateway `GET /runs/{job}/stream`). Because the `agent` request item
    carries `job`, a subscribed client opens that run stream and renders the agent's work live — tool
    calls, partial tokens — inside the message card. This is the shipped run-feed surface, reused; the
    channel adds no stream.
  - *State (durable):* on completion the worker posts one `agent_result` item (`a:<request-id>`) with the
    final answer. That item is what persists in `history` and what a later scroller sees; it supersedes the
    live feed. The agent-run job transcript remains the authority for the full step-by-step run.

**Where the work runs:** a host-side channel **agent worker**, hooked into the `post` path exactly where
`run_if_query` is (`channel/post.rs`), under the poster's principal. Host-side (not browser) so the
`agent.invoke` gate and every downstream tool's `caps::check` are never client-trusted, and so another
extension or agent can ask the same way (rule 7). It is a thin seam: parse the `kind:"agent"` payload →
start the run via the existing invoke path with the selected `runtime` → on completion post the answer.

**Runtime selection is the seam, not a branch.** The worker passes the payload's `runtime` straight into
the existing `AgentInvokeRequest.runtime` → `RuntimeRegistry` (absent → default, known → entry,
named-unknown → error). No `if external {…}`; the channel worker is identical whether the run is the
in-house loop or an external ACP subprocess. When #3 lands and an external profile is granted, an
`agent` item with `runtime:"open-interpreter"` drives it with zero channel-side change.

**Alternative considered — run the agent client-side / post only the answer.** Rejected for the same
reason query rejected it: it moves the `agent.invoke` gate and every tool's capability check into the
browser (a client could fabricate an `agent_result` for tools it can't call), and it loses the durable
request item and the audited run transcript. Posting the request and letting a host worker drive a real
durable run keeps every gate host-side and the audit complete.

**Alternative considered — inline-to-completion like the query worker.** Rejected: an agent run is not
bounded like a SELECT; blocking the poster's `post` connection for the length of a multi-tool run is
unacceptable, and it throws away the resumability the agent-run job already gives (edge disconnect mid-run
would lose the answer). Spawning the durable job is the correct reuse.

## How it fits the core

- **Tenancy / isolation:** unchanged from channels/agent-run — the run job carries `ws`; store reads use
  the workspace namespace; the run stream subject is workspace-walled (`ws/{id}/run/{job}/events`). A
  ws-B member can neither post into a ws-A channel nor watch a ws-A run.
- **Capabilities:** asking an agent requires **two** grants, checked in order — channel
  `bus:chan/{cid}:pub` to post the `agent` item, then `mcp:agent.invoke:call` when the worker starts the
  run (under the poster's principal, so a member without the invoke grant gets an opaque `agent_error`).
  Inside the run, **every tool the agent calls re-runs `caps::check`** under the derived principal and is
  subject to `agent_policy:{ws}` (agent-run Part 2) — the channel adds no new authority; the agent can do
  exactly what the asker is granted, nothing more. The named-unknown/ungranted `runtime` path is opaque.
- **Placement:** either. The worker is symmetric host code (no `if cloud`, no `if external`); it runs
  wherever the channel and the agent runtime run. Edge vs cloud, in-house vs external, are config/role/
  cargo-feature — never a code branch here.
- **MCP surface** (API shape, §6.1):
  - **Create (the only write):** no *new* tool — asking an agent is `channel.post` with a `kind:"agent"`
    payload. Reuses the existing `post` verb + `pub` gate; the worker reuses `agent.invoke`. No
    `agent_ask` tool is added: the channel `post` verb already *is* the create, the same decision query made.
  - **Get / list:** none new — `channel.history` returns all items including `agent`/`agent_result`; the
    UI filters by `kind`. The live run detail is the existing read-only `agent.watch`.
  - **Live feed:** none new — the channel SSE `event: message` carries the `agent`/`agent_result` items;
    the *run* detail streams over the existing `GET /runs/{job}/stream`. Two shipped streams, composed;
    no new SSE event type.
  - **Batch:** N/A this slice. One `agent` item is one run.
- **Data (SurrealDB):** the `agent`/`agent_result` `Item`s persist to `lb_inbox` exactly like any channel
  message — same table, same workspace namespace, no new table. The run itself is the existing `job:{id}`
  record (agent-run) — also no new table. The channel item stores the *final answer* (bounded, see Risks);
  the full transcript stays in the job, not duplicated into the channel.
- **Bus (Zenoh):** channel motion reuses `chan/{cid}/msg/**`; run motion reuses the agent-run
  `run/{job}/events` subject. Both are replay-class (persisted before publish). No new subject.
- **Sync / authority:** node-local channel store + the durable run job. Offline: an `agent` item posted
  offline is durable; the worker starts the run when the node (and model provider) are reachable. A run
  interrupted by edge disconnect **resumes** via the agent-run job — the answer is not lost.
- **Secrets:** none new client-side. The model key never leaves the gateway/secret store; the worker
  starts a run, it never handles a provider credential (model-routing #4 governs that for external runtimes).
- **State vs motion:** the request item and the answer item are state (inbox) first, motion (bus) second —
  same order as every channel post. The live run feed is motion over the durable job (state).
- **Stateless worker:** the agent worker holds no durable state — the request is in the inbox item, the
  run is in the job, the answer is an inbox item. Hot-reload safe; a worker restart mid-run is covered by
  the job's own supervision/resume (agent-run / external-agent #5).

## Example flow

1. Alice (holds `bus:chan/ops:pub` and `mcp:agent.invoke:call`) opens `/` in channel `ops`, picks the
   agent command (visible because she holds `mcp:agent.invoke:call` — the catalog gate), types the goal
   "What changed in the deploy logs in the last hour?", and leaves the runtime dropdown on its default.
   The palette (`channels-command-palette`) mints a run id and builds the payload via `onSendAgent`.
2. The UI posts an `Item` with body `{ kind:"agent", goal:"What changed…", job:"run-abc" }` via the
   existing channel `post`. The `pub` gate passes; the item persists and publishes; everyone in `ops` sees
   the question appear (rendered as an agent card in a "running" state, not raw JSON).
3. The host agent worker sees the `kind:"agent"` item and starts a durable run via `agent.invoke` under
   Alice's principal, `runtime` absent → the default in-house loop, `job = run-abc`. The `agent.invoke`
   grant is checked — pass. `post` has **already returned**; the run proceeds asynchronously.
4. The UI, seeing the `job` on the request item, opens `GET /runs/run-abc/stream`. The card fills in live:
   the agent calls `federation.query` on the logs source (its own grant re-checked under Alice's
   principal), streams partial reasoning, calls another tool — Alice watches it work.
5. The run completes. The worker posts a second `Item`, `kind:"agent_result"`, `{ goal, runtime:"default",
   job:"run-abc", answer:"Three deploys; the 14:02 one rolled back — …" }`, under a system identity
   (`system:agent-worker`, no `pub` re-check — the host is posting its own answer). It persists and
   publishes; the card settles to the final answer.
6. Bob (channel `sub` only) scrolls back tomorrow and sees Alice's question and the agent's answer in
   durable history. Bob cannot ask his own (no `pub`); if he tries, the `post` gate denies opaquely.
7. Carol asks with `runtime:"open-interpreter"` but the ws has not been granted that external profile (or
   #3 has not shipped): the worker posts an opaque `agent_error` "agent not permitted" — no leak of
   whether the runtime exists.

## Testing plan

Per `scope/testing/testing-scope.md`; no mocks — real store (`mem://`), real bus, real gateway, the real
agent-run job path. The **one** permitted fake stays the model provider behind the gateway (the existing
`MockProvider`) — the agent loop, the run job, the stream, and the channel are all real (rule 9).

- **Capability deny (mandatory):**
  - member with channel `sub` but not `pub` posting an `agent` item → host `post` deny (opaque).
  - member with channel `pub` but **no** `mcp:agent.invoke:call` grant → worker posts an `agent_error`
    "agent not permitted"; assert it does not reveal whether a runtime exists.
  - an `agent` item naming an unknown/ungranted `runtime` → opaque `agent_error` (reuse the
    `RuntimeRegistry` named-unknown→error + the #3 grant-gate); assert no runtime-existence leak.
  - a tool the agent tries that the poster's derived principal lacks → denied at `caps::check` inside the
    run and surfaced as a run error, never executed (reuse agent-run Part 2 deny path).
- **Workspace isolation (mandatory):** ws-B identity cannot post an `agent` item into a ws-A channel,
  cannot read ws-A `agent_result` history, and cannot watch a ws-A run stream. Mirror the ws-A/ws-B
  structure in `gateway_routes_test.rs` / the agent-run watch test.
- **Re-entrancy (unit + integration):** only `kind:"agent"` triggers the worker; the worker's own
  `agent_result` / `agent_error` items (and plain chat, and `query*` items) do **not** — assert no second
  run is spawned from a result item (the infinite-loop guard, exactly as query tests it).
- **Async spawn (integration, real gateway):** posting an `agent` item returns from `post` **before** the
  run completes (assert the request item lands immediately; the `agent_result` appears in `history` only
  after the run finishes). Assert the `agent_result` streams live over the channel SSE as `event: message`.
- **Runtime seam (integration):** the same channel `agent` path drives the default runtime; a second
  `agent` item with a registered alternate `runtime` id drives that one via the seam with **no channel-side
  code change** (mirror the external-agent swap test once #3/a test profile is available).
- **Resume (integration):** a run interrupted mid-flight resumes and still posts exactly one
  `agent_result` (no double-post, no re-spend) — reuse the agent-run resume test harness.
- **UI (real gateway, `*.gateway.test.tsx`, no fakes):** asking in `ChannelView` renders the agent card in
  a running state, fills it from the real run stream, then settles to the final answer when the
  `agent_result` item streams in; an `agent_error` renders an inline opaque error. Seed via the real
  gateway, per rule 9.

## Risks & hard problems

- **Answer size.** The final answer lives inside a channel `Item` in the inbox. A verbose agent could
  bloat history/the bus frame. **Mitigation:** cap the persisted `answer` (reuse the query worker's
  ≤256 KB posture) with a `truncated` flag and "view full run" linking to the job transcript; the full
  step-by-step stays in the job, not the channel item. Decide the cap before building (Open questions).
- **Worker identity & re-entrancy.** The worker posts `agent_result` items, which are channel posts — it
  must never treat its own output as a new request. Only `kind:"agent"` triggers work; guard explicitly
  and test, exactly like the query worker (an infinite loop is one absent guard away).
- **Long / hung runs.** Unlike a SELECT, a run can run for minutes or hang. This is why it is now a
  durable job drained by a background reactor (the background slice shipped this): the run detaches from
  `post` and survives a restart. **Supervision (wall-time ceiling + kill/reap) is still open** — a run
  that hangs must eventually post an `agent_error`, not leave the card spinning forever; that is the
  remaining external-agent #5 half. (A run that *dies* on a fault already posts an `agent_error`.)
- **Duplicate spawn — RESOLVED by the background slice.** The reactor drains a durable enqueue job, and
  `drive_queued_run` short-circuits if the correlated answer item (`a:<run_job>`) already exists — so a
  re-drain (tick overlap, redelivery, restart mid-queue) never starts a run twice or double-posts. An
  in-process `in_flight` set additionally avoids spawning a second drive while the first is still running.
- **Model provider required.** Any real run needs a live model (STATUS notes `agent_invoke` gateway wiring
  was deferred precisely because it needs a real provider, no mock). The channel worker inherits that: with
  no provider configured the run posts an honest `agent_error`. Wiring a real provider/gateway model is a
  prerequisite for the end-to-end demo (config, not code in this scope).
- **External-runtime safety is NOT this scope's gate.** Reaching an external ACP agent from a channel is
  unsafe until external-agent #3 (the capability wall) ships. This scope must fail-closed to the in-house
  runtime until an external profile is both feature-compiled and granted — never silently reach a
  subprocess. Assert the closed default in tests.

## Open questions

Resolved in the build session ([channels-agent-session.md](../../sessions/channels/channels-agent-session.md)):

- **Worker trigger — DECIDED (v1): inline in `channel.post`, awaited. SUPERSEDED (background slice):
  the worker now ENQUEUES a durable job and returns; a background reactor drains it.** v1 hooked at the
  same point as `run_if_query` (the faithful reuse of the proven worker pattern; worked end to end) but a
  long run blocked the poster's `post` and closing the tab mid-run cancelled it. The
  [background slice](../../sessions/channels/channels-agent-background-session.md) (run-lifecycle #5)
  fixed both: `run_if_agent` writes a durable `channel-agent-run` enqueue job (carrying the poster's
  sub+caps) and returns at once; `spawn_agent_reactors` (twin of `spawn_flow_reactors`) drains pending
  jobs off the connection and drives each via `drive_queued_run` under the reconstructed poster. The run
  now survives the tab closing AND a node restart, and is idempotent on `a:<run_job>` (a re-drain never
  re-runs / double-posts). The request item is still published BEFORE the run drives, so a watcher sees
  the run start live. **Supervision (wall-time ceiling + kill/reap of a hung run) is the remaining #5
  half.**
- **Who mints `job` — DECIDED: the UI** (via `newRunId`, as `AgentView` mints `jobId`), so a client can
  subscribe to the run stream the instant the request item lands.
- **Answer cap — DECIDED: ≤256 KB** with a `truncated` flag (mirrors the query worker;
  `agent_worker.rs::AGENT_MAX_BYTES`, char-boundary safe). "View full run" links to the run stream/job.
- **In-channel per-tool Allow/Deny card — DECIDED: excluded from v1** (relies on `agent_policy` Deny/Allow
  only). The interactive first-settle card is a fast-follow.
- **Palette command shape — REVISED (2026-07-01): a first-class `agent.invoke` palette command**, not a
  `/agent [@runtime] <goal>` chat string. The rendered composer is the `CommandPalette`, so the agent is a
  real `tools.catalog` descriptor (gated by `mcp:agent.invoke:call` via the catalog's per-tool
  `authorize_tool` — the descriptor name IS the gate) whose `runtime` arg is a **dropdown** backed by the
  `agent.runtimes` read verb (#5), default preselected. Still UI-built into the `kind:"agent"` payload;
  the host still never parses chat text. See
  [agent-runtimes-scope.md](../external-agent/agent-runtimes-scope.md). (The original `/agent [@runtime]`
  string parsed by `parseAgentCommand` on `MessageComposer` was orphaned — that composer was never
  rendered — and is DELETED.)
- **Skill/persona selection — DEFERRED:** v1 omits it; a later slice adds a persona picker (grant-gated
  `load_skill`).

**New (surfaced during the build, tracked for follow-up):**
- **Live run-feed in the card** — the "both" streaming half. The run publishes `RunEvent`s and the request
  carries `job`; the UI subscription + live rendering is the agent-run Part 3 "run-feed UI" follow-up.
- **External-agent #3/#4 are the gates for a *production* external run** — v1 drives Open Interpreter →
  Z.AI directly via `ZAI_API_KEY` for the demo; #3 (capability-wall) and #4 (model-routing through our
  gateway) must land before an untrusted external agent runs in anger.

## Related

- `scope/channels/channels-query-charts-scope.md` — the inline-worker + kind-tagged-payload pattern this
  mirrors (the agent worker is its sibling; the key divergence is spawn-a-durable-job vs inline-execute).
- `scope/channels/channels-command-palette-scope.md` — the `/` + `@` surface that composes the
  `kind:"agent"` item; this is its next tenant after query.
- `scope/channels/channels-scope.md` — the channel registry/history/stream surface this builds on.
- `scope/agent-run/agent-run-scope.md` — the durable run job, `RunEvent` stream, `agent.watch`/SSE, and
  per-tool policy this reuses wholesale for the live feed + durable answer + resume.
- `scope/external-agent/external-agent-scope.md` (+ `runtime-seam-scope.md`, `capability-wall-scope.md`) —
  the `AgentRuntime` seam this selects through; #3 is the gate before any external runtime is reachable
  from a channel.
- `ui/src/features/agent/AgentView.tsx` / `ui/src/lib/agent/agent.api.ts` — the existing `agent.invoke`
  client seam (the `runtime` field lands here too).
- `rust/crates/host/src/channel/query_worker.rs` / `post.rs` — the worker + hook point to sit beside.
- `README.md` §6 (channels/bus), §6.9 (jobs), §6.16 (shared agents), §6.14/§6.15 (gateway run-SSE), §7
  (tenancy), §3 (the non-negotiables this honors).
- `docs/public/channels/channels.md` — promotion target on ship.
</content>
</invoke>
