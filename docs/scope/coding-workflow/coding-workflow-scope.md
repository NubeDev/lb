# Coding-workflow scope — the worked example, end to end

Status: scope (the ask). Promotes to `public/coding-workflow/` once the S6 slice ships.

> Read with: `../../vision/0002-coding-agent-workplace.md` (the design probe this implements),
> `../agent/agent-scope.md` (the S5 central agent it drives), `../jobs/jobs-scope.md` (the durable
> session), `../inbox-outbox/outbox-scope.md` (the must-deliver effects), `../files/files-scope.md`
> (the shared scope doc), `../../README.md` §6.16, §6.10, §6.9.

The S6 worked example: a GitHub issue becomes an inbox item; a workflow asks the S5 central agent
to triage it and draft a scope doc shared to the team; an **approval** inbox item **genuinely
gates** a durable **coding job**; on approval the job runs, streams progress to a channel, and
routes **every external effect through the transactional outbox with retry**. Nothing here is a new
kernel feature — it is a *composition* of inbox + agent + jobs + outbox + assets, proving the core
can express a real product (the substitutability test, vision §4). The core never learns the words
"coding agent."

## Goals

- **Issue ingress** — a `github-bridge` path writes a normalized inbox item tagged `source:github`,
  `needs:triage`, idempotently (a retried webhook upserts one item).
- **Agent triage** — the workflow calls the **S5 agent over MCP** (the same routed path an edge
  user would use) to read the issue + related docs/channel context and draft a **scope doc**,
  shared to the team via the S4 `share_doc`. The workflow does the reasoning *through the agent*,
  holding no durable state of its own (stateless, §3.4).
- **A genuine approval gate** — the workflow writes a `needs:approval` inbox item for a
  team/user; the coding job **cannot start** until that item is resolved `approved`. A rejected or
  unresolved approval leaves the job unstarted. This is the load-bearing behavior of the exit gate.
- **A durable coding job** — on approval, a S5 `lb-jobs` session starts; progress streams to a
  channel (motion); the job survives the approver disconnecting.
- **Every external effect through the outbox** — PR, comment, notify, sync-publish, and any
  downstream workflow-start are written as `Effect` rows in the **same transaction** as the domain
  change, then relayed at-least-once with retry (outbox scope). The job never calls GitHub directly.

## Non-goals (S6)

- A real GitHub client / webhook server — `github-bridge` is a thin in-test source+sink (the only
  external mocked, testing §3); the canonical receiver/transport is S7.
- A real coding agent that edits files / runs tests — the S5 generic agent + the mock provider
  drive the loop; the *coding* tools are out of scope (the slice proves the orchestration, not a
  code editor). The job's "work" is the agent loop + outbox effects.
- Multi-issue concurrency, duplicate-detection heuristics, reviewer routing policy — the flow is
  one issue, one approval, one job. Routing is a tag (`route:team:reviewers`), not a policy engine.
- The `coding-workflow`/`github-bridge` as *installed wasm extensions* — at S6 they are **host
  services** (like `agent`/`channel`/`assets`), because the orchestration must drive `caps::check`,
  the agent loop, jobs, and the transactional outbox — all host-internal seams (same reasoning as
  the agent being a host service, agent scope). Packaging them as registry artifacts is S7.

## Intent / approach

**The workflow is a host service that orchestrates; it owns no durable state.** Every fact it acts
on lives in a record: the issue in the inbox, the approval in the inbox (with a resolution facet),
the conversation in the agent's job, the effects in the outbox. Kill the workflow mid-flight and
another invocation resumes from those records — exactly the stateless-extension guarantee. It sits
beside `agent/` as `workflow/`, one verb per file (FILE-LAYOUT).

**The approval gate is data, not a special primitive** (vision §5 finding): an approval is an inbox
item with `needs:approval` and a **resolution facet** (`approve|reject|defer` + actor + ts). The
inbox grows one small sibling record (`Resolution`) — `Item` stays stable. `start_coding_job`
reads the resolution and **refuses unless it is `approved`** — that refusal *is* the gate. No
bespoke approval table, no workflow-state machine. **Rejected:** a dedicated `approval` table — it
would duplicate the inbox's normalized-item shape and the routing/unread machinery.

**Every external effect is transactional-outbox, never direct** (the S6 driver, outbox scope): the
job calls a workflow verb that, in one transaction, writes the domain change and the `Effect` row.
The relay delivers at-least-once; the receiver dedups on `idempotency_key`. This is what the exit
gate means by "through the outbox with retry." **Rejected:** the job publishing effects on the bus
— fire-and-forget can drop a must-deliver effect on a disconnect (the exact failure §6.10 closes).

**The agent is reached over MCP, identically to an edge user** (vision §3, edge-invoke-central
parity): the workflow calls the same `invoke` path; it is just another MCP caller. This keeps the
agent's capability scoping (`agent ∩ caller`) and the routed seam unchanged — the workflow doesn't
get a privileged back channel.

## How it fits the core

- **Tenancy / isolation:** every record (inbox item, resolution, job, outbox effect, scope doc) is
  ws-scoped; the workflow verbs select the namespace via the underlying crates. A ws-B workflow
  never sees ws-A's issues/approvals/jobs/effects — the mandatory isolation test, across store +
  MCP.
- **Capabilities:** new MCP grants gate the workflow surface — `mcp:workflow.ingest_issue:call`,
  `mcp:workflow.triage:call`, `mcp:workflow.request_approval:call`,
  `mcp:workflow.resolve_approval:call`, `mcp:workflow.start_job:call`. The deny path: a caller
  without the grant cannot ingest / triage / approve / start. The job's *agent* call re-runs the
  S5 intersection (`agent ∩ caller`); the job's *effects* are gated by the workflow grant. Two
  independent surfaces, both enforced.
- **Placement:** *either*, hub-default (the agent + relay live on the hub). No `if cloud` —
  placement is which node mounts the workflow + relay, config only (symmetric nodes).
- **MCP surface:** exposes `workflow.*` (host-native bridge, like `assets.*`/`agent.invoke`).
  Consumes `agent.invoke` (drives the agent), `assets.put_doc`/`share_doc` (the scope doc), the
  inbox verbs, the jobs verbs, the outbox verbs.
- **Data (SurrealDB):** the inbox `Item` (+ new `Resolution` sibling), the `job` record, the new
  `outbox` table, the scope `doc`. All state; the only motion is progress streaming.
- **Bus (Zenoh):** two classes, kept distinct (§6.2). **Fire-and-forget:** progress chatter to the
  channel ("triaging", "job started", "PR queued"). **Must-deliver:** every external effect — NOT
  on the bus, through the **outbox** (the whole point of S6). The routed `agent.invoke` rides the
  S3 queryable.
- **Sync / authority:** the hub is authoritative for the job + outbox; an edge converges on the
  `(table,id)` upserts. Offline: the approver may disconnect after approving; the hub runs the job
  and relays effects; the edge reads durable progress on reconnect.
- **Secrets:** N/A to the workflow (the target adapter holds the GitHub credential at S7; the
  effect row carries no secret).

## Example flow (the exit gate, concretely)

1. **Issue arrives.** `ingest_issue` writes an inbox `Item` in channel `triage`, tagged
   `source:github needs:triage` (idempotent on the issue id).
2. **Triage.** `triage` calls the S5 agent (`invoke`) to read the issue (+ a granted skill / shared
   doc substrate) and produce a scope-doc body; the workflow `put_doc`s it and `share_doc`s it to
   team `backend`, posts a summary to `#issue` (motion).
3. **Approval requested.** `request_approval` writes a `needs:approval` inbox item routed to team
   `reviewers`, referencing the scope doc and the proposed job.
4. **The gate.** `start_coding_job` is called — and **refuses** (`AwaitingApproval`) because the
   approval is unresolved. *This refusal is the gate.*
5. **A reviewer approves.** `resolve_approval` writes the resolution `approved` (actor + ts).
6. **The job starts.** `start_coding_job` now passes the gate, creates the `lb-jobs` session, runs
   the agent loop, and streams progress to the channel.
7. **Effects through the outbox.** As the job produces external effects (open PR, comment), each is
   written `write_with_effect` — the job step AND the `Effect` row in one transaction.
8. **Relay with retry.** The hub relay delivers each effect to its `Target`; a failed delivery
   retries next pass; the receiver dedups on `idempotency_key`. Nothing is lost or double-sent.
9. **Results land back.** Completion summary to the channel; the scope/result docs are shared; a
   `needs:review` follow-up inbox item can re-enter the loop. The job is `Done`.

## Testing plan

Mandatory categories (testing §2) — the S6 gate:

- **Capability-deny** (§2.1): each `workflow.*` verb denied without its grant (ingest/triage/
  request_approval/resolve_approval/start_job). The agent call inside the job still re-checks the
  S5 intersection (covered by the agent tests; here we assert the workflow gate).
- **Workspace-isolation** (§2.2): a ws-B caller cannot see ws-A's inbox issues / approvals / job /
  outbox effects / scope doc — across store + MCP. A ws-B relay delivers no ws-A effect.
- **Offline / sync** (§2.3): **the outbox delivers at-least-once** — an effect survives a target
  failure and is delivered on retry (never lost); a duplicate delivery is a no-op (dedup on
  `idempotency_key`, never double-sent); the `write_with_effect` transaction is atomic (a forced
  failure leaves neither the job step nor the effect). Plus: the job survives the approver
  disconnecting and resumes (reuses the S5 resume path).
- **The gate itself** (the headline behavior): `start_coding_job` **refuses** while the approval is
  unresolved/rejected and **succeeds** once approved — the same job id, so the gate is genuine, not
  cosmetic. No job record exists before approval; exactly one after.
- E2E: the full flow (ingest → triage → approval → job → outbox relay) in one test, the exit gate.
- Unit: the resolution facet transitions; the gate predicate; injected clock/ids (determinism).

## Risks & hard problems

- **The gate must be checked at the job-start chokepoint, not advisory.** If `start_coding_job`
  reads the approval but a second path could start the job without it, the gate is fiction. There
  is exactly one job-start verb, and it reads the resolution before creating the record — no second
  path (mirrors the single `caps::check` chokepoint discipline).
- **Effects must be transactional or the durability promise is void** — see outbox scope. The
  workflow uses `write_with_effect` for every effect; a direct enqueue would reopen the window.
- **Stateless orchestration under interruption.** The workflow must hold nothing durable; every
  resumable fact is a record. The test kills/recreates the flow between steps to prove resumption
  from the inbox/job/outbox.
- **Edge-invoke parity.** The workflow's agent call must be the identical routed path an edge user
  uses — a privileged shortcut would diverge online/offline behavior (vision §5.7).

## Open questions

- **Resolution richness:** does `Resolution` carry just `{decision, actor, ts}`, or also a comment
  / a required-approver-count? S6 ships the minimal triple; quorum/multi-approver is a follow-up.
- **Who calls `start_coding_job`** — the approver's UI action directly, or a workflow reactor
  watching the inbox for a resolution? S6 exposes the verb (UI/test drives it); the LIVE-query
  reactor is the latency optimization (deferred, same as the outbox relay's LIVE push).
- **Idempotency-key derivation** for effects — convention (`<action>:<issue>`) vs explicit caller
  arg. S6 takes it as an explicit arg (the caller owns the key's stability); a derivation helper is
  a follow-up.
- **Packaging as wasm extensions** — when `coding-workflow`/`github-bridge` move from host services
  to installed artifacts (S7 registry). The host-service shape is the S6 decision; revisit at S7.
  **RESOLVED (S7) for the `github-bridge`:** packaged as a pure-transform Tier-1 wasm artifact
  (`../extensions/github-bridge-scope.md`) — the normalizer is a sandboxed guest installed through
  the registry; the orchestrator stays a host service (the §3.4 seam reasoning, re-confirmed).
- **Follow-up inbox loop** — `needs:review` re-entry (vision §3 step 9) is sketched, not built at
  S6 (one pass through the flow).

## Related

- `../../vision/0002-coding-agent-workplace.md` — the design probe (every step here maps to a
  numbered step there).
- `../agent/agent-scope.md`, `../jobs/jobs-scope.md`, `../inbox-outbox/outbox-scope.md`,
  `../files/files-scope.md`, `../inbox-outbox/inbox-outbox-scope.md`.
- README `§6.16` (shared AI agents & workflow extensions), `§6.10` (inbox/outbox), `§6.9` (jobs),
  `§6.12` (docs/skills), `§7` (tenancy).
</content>
</invoke>
