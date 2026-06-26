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

## Running it end to end (S7)

The flow runs as a live process, not just a sequence of verbs a test drives:

- **Ingress.** `lb-role-github-webhook` (`POST /webhook/{tenant}`) HMAC-verifies a real GitHub
  delivery and drives `ingest_via_bridge` → the `needs:triage` item. Multi-tenant: one process fronts
  many workspaces, each with its own secret (see `../extensions/extensions.md`).
- **Auto-start on approval.** `react_to_approvals` — a durable scan over approved resolutions — starts
  the coding job the moment its approval lands `Approved`, with no manual `start_job`. The PR effect it
  queues carries the structured `{repo, head, base, title, body}` payload (a `PrSpec` recorded at
  approval), so a real PR can open. Idempotent on a deterministic job id (re-scan → one job, one PR).
- **The driver.** `lb-role-github-workflow` ticks the reactor + the outbox relay per workspace on an
  interval (`run_workflow_loop` / `drive_once`) — reactor first, so a freshly-approved job's PR ships
  the same tick. The host owns the verbs; the role owns the cadence; the GitHub HTTP `Target`
  (`lb-role-github-target`) is supplied behind the trait. `now` is injected (wall-clock only at the
  binary). A tick over one workspace never touches another.
- **Dynamic workspaces.** The set of serviced workspaces is a durable **directory**
  (`register_workspace` / `deregister_workspace`, a reserved-namespace record) the driver re-reads each
  tick (`run_directory_loop`) — a workspace is onboarded or retired **without restarting the node**, and
  the set survives a restart. The directory is secret-free; per-tenant webhook secrets ride `lb-secrets`
  (the paired follow-up).
- **Mounted by config.** The `node` binary spawns the webhook server + the driver loop when the
  environment configures them (`LB_WORKFLOW_WS`, `LB_WEBHOOK_ADDR/SECRET`, `LB_GITHUB_API/TOKEN`) —
  config, never an `if cloud`. Absent config, the binary is the solo node.

So a webhook delivery now flows **issue → triage → approval → JOB → PR** as a running service, end to
end. Open follow-ups: a dynamic workspace/tenant directory (hot-add without a restart), a LIVE-query
driver (instant pickup), `lb-secrets`-backed secrets, and a real login→principal session.

## UI

A `WorkflowView` + `workflow.api` client mirroring the verbs, with a faithful in-memory fake
exercising the capability gate, the approval gate, and the outbox. See `../frontend/frontend.md`.
</content>
