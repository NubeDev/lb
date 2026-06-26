# Coding workflow (as built)

The S6 worked example, end to end — a product built **entirely as a composition** of core
primitives (vision `0002`). The core never learns the words "coding agent": it knows inbox items,
resolutions, jobs, outbox effects, channels, docs, capabilities, and a routed MCP namespace.
Promoted from `../../scope/coding-workflow/coding-workflow-scope.md`.

## The flow

```
GitHub issue ──ingest_issue──▶ INBOX (triage, needs:triage)
                                   │  triage (drives the S5 agent over MCP)
                                   ▼
                         scope DOC ──share_doc──▶ team   + summary to #channel (motion)
                                   │  request_approval
                                   ▼
                         INBOX (approvals, needs:approval) ──resolve_approval──▶ Resolution
                                   │  start_coding_job  ← THE GATE: only if Approved
                                   ▼
                         durable JOB (lb-jobs)  + progress to #channel (motion)
                                   │  emit_effect (job step + effect, ONE transaction)
                                   ▼
                         OUTBOX (pending) ──relay_outbox──▶ Target (at-least-once, retry, dedup)
```

## The host `workflow` service

Beside `agent`/`channel`/`assets` — a host service (not a wasm extension at S6) because the
orchestration drives `caps::check`, the agent loop, jobs, and the transactional outbox. It holds
**no durable state** (stateless extensions, §3.4): every fact is a record. One verb per file:

- `ingest_issue` — write the inbox `needs:triage` item (idempotent on the issue id — replay-safe
  webhook ingress).
- `triage` — drive the **S5 central agent** over the same `invoke` path an edge user uses
  (edge-invoke parity), then `put_doc` + `share_doc` the draft and post a channel summary.
- `request_approval` — write the `needs:approval` inbox item routed to a team.
- `resolve_approval` — write the `Resolution` (approve/reject/defer; `actor` is the audited sub).
- `start_coding_job` — **THE GATE**: refuse (`AwaitingApproval`) unless the approval is `Approved`,
  creating nothing; on approval create the durable job, stream progress, and route the PR through the
  outbox. The refusal *is* the gate — no job record exists before approval.
- `emit_effect` — the transactional must-deliver write (the job step + the effect, one transaction).
- `relay_outbox` — deliver `pending` at-least-once through a `Target`, marking each outcome.

## The approval gate

An approval is **data, not a primitive**: an inbox `Item` tagged `needs:approval` plus a
`lb_inbox::Resolution` sibling (`{item_id, decision, actor, ts}`). `start_coding_job` reads the
resolution and starts the job **only on `Approved`** — a rejected, deferred, or missing resolution
leaves the job unstarted, with no record. Exactly one job-start chokepoint reads the gate; there is
no second path that could start the job without it.

## MCP surface

`workflow.*` is reached through the one MCP contract (`mcp:workflow.<verb>:call`, workspace-first)
via a host-native bridge (`call_workflow_tool`) — `ingest_issue`, `request_approval`,
`resolve_approval`, `start_job`. `triage` is not bridged (it needs a `ModelAccess`, like the agent's
`invoke`). Two gates always: the MCP grant, then the verb's own gate (`start_job` re-checks the
approval).

## Guarantees

- **Capability-deny:** each verb refuses without its grant; `start_job` is denied at the capability
  gate *before* the approval gate.
- **Workspace-isolation:** every record (issue, approval, job, effect, doc) is ws-scoped; a ws-B
  caller sees none of ws-A's, and a ws-B relay delivers no ws-A effect — across store + MCP.
- **The gate is genuine:** no job before approval; exactly one after; a rejected approval starts
  nothing.
- **Every external effect through the outbox with retry** — see `../inbox-outbox/inbox-outbox.md`.

## UI

A `WorkflowView` + `workflow.api` client mirroring the verbs, with a faithful in-memory fake
exercising the capability gate, the approval gate, and the outbox. See `../frontend/frontend.md`.
</content>
