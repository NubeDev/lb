# 0002: Worked example — the Coding Agent Workplace extension

A concrete, end-to-end walkthrough of one product built **entirely as an extension** on
the Lazybones core. Nothing below is special-cased in the platform: the "Coding Agent
Workplace" is just a composition of core primitives — workspaces, teams, users, channels,
docs, skills, inbox, outbox, jobs, MCP tools, and a central AI agent.

Read this as a design probe. If the core primitives can express this product cleanly,
the core is the right shape. Every place this example reaches for something the core does
not yet offer is a finding for the coding scope, not a feature to bolt onto the kernel.

> **The one thing to take away:** the core never knows the words "coding agent." It knows
> inbox items, outbox rows, jobs, channels, docs, capabilities, and a routed MCP namespace.
> The product is the *arrangement* of those, shipped as installable artifacts.

---

## 1. What the extension is

A workspace installs a small bundle of cooperating extensions:

| Extension | Tier / placement | Role |
|---|---|---|
| `github-bridge` | WASM, `either` | Receives GitHub webhooks, normalizes them into inbox items, and delivers outbox effects back to GitHub. |
| `coding-workflow` | WASM, `cloud-only` | The orchestrator. Watches inbox, drives the AI agent, manages approvals, starts jobs, writes outbox rows. Holds **no** durable state of its own. |
| `coding-agent` | central AI actor, `cloud-only` | The workspace-scoped AI agent hosted on the hub. Exposed over MCP; callable by edge users and by `coding-workflow`. |
| `coding-skills` | docs/skills assets | Versioned skills the agent may load *only when granted* (e.g. "triage rubric", "scope-doc template", "repo conventions"). |

These are ordinary registry artifacts (§6.4 of the core scope): pulled, signature-verified,
cached, and instantiated per workspace with only the capabilities the workspace admin
granted at install. "Central agent" is a placement and a role, not a privilege tier — its
effective access is still workspace-first (§6.6, §7).

### How it maps to the tenancy model

- **Workspace** `acme` = the tenant = the hard wall. One SurrealDB namespace, one
  `ws/acme/**` bus prefix, its own secrets. Every artifact below lives inside it. The shipped
  workspace directory/session behavior is documented in `../public/workspace/workspace.md`.
- **Teams** `backend`, `reviewers` = membership groups used for assignment, mentions, and
  approval routing. Flat and overlapping (§7).
- **Users** = global identities who are members of `acme`. Some are on **edge** nodes
  (laptops, offline-capable); the agent and workflow run on the **cloud hub**.
- **Channels** `#issue-2451`, `#backend` = bus subjects (`ws/acme/chan/{cid}/**`) with
  messages persisted to SurrealDB. The agent posts progress here; humans discuss here. The shipped
  channel registry/history/stream behavior is documented in `../public/channels/channels.md`.

---

## 2. The actors at a glance

```
GitHub ──webhook──▶ github-bridge ──writes──▶ INBOX (needs:triage)
                                                  │
                                       coding-workflow watches
                                                  │
                                                  ▼
                                   calls coding-agent over MCP ◀── edge user can call too
                                                  │
                  reads: docs · skills · #channel history · repo (via granted MCP tools)
                                                  │
                                writes scope DOC ──shared with team `backend`
                                                  │
                                  creates INBOX item (needs:approval) → team `reviewers`
                                                  │
                                            human approves
                                                  │
                                   starts JOB = durable remote coding session
                                                  │
              streams progress ──▶ CHANNEL #issue-2451   uses granted MCP tools/skills
                                                  │
                external effects ──▶ OUTBOX ──relay──▶ GitHub comment / PR / notify / sync
                                                  │
                          results ──▶ DOCS · MESSAGES · GitHub updates · follow-up INBOX
```

---

## 3. The flow, step by step

### Step 1 — A GitHub issue arrives

A webhook hits the `github-bridge` extension. It does exactly one core-level thing:
it writes a **normalized inbox item** (§6.10) in a single SurrealDB transaction:

```
inbox.item {
  source:  "github",
  type:    "issue.opened",
  payload: { repo: "acme/api", number: 2451, title: "...", body: "...", url: "..." },
  tags:    [ source:github, repo:acme/api, kind:issue, needs:triage ],
  read:    false,
  ts:      ...
}
```

The bridge knows nothing about coding workflows. It is a generic source adapter: any
external system (email, CI, chat) deposits into the **same inbox shape**. The
`needs:triage` tag is the only contract between the bridge and whatever consumes it.

> *Scope finding:* webhook ingress needs a stable, idempotent path (replay-safe on retry).
> The bridge should be `either`-placed so a self-hosted edge can receive webhooks on a LAN,
> but the canonical receiver is the hub.

### Step 2 — The workflow sees the item and asks the agent to triage

`coding-workflow` subscribes to inbox items matching `needs:triage` (a tag query, §6.11 —
LIVE query for instant pickup, with a durable scan as the backstop since LIVE is ephemeral,
§6.1/§6.2). On a match it does **not** do the reasoning itself. It calls the central
`coding-agent` through the routed MCP namespace (§6.5):

```
mcp.call coding-agent.triage {
  inbox_ref: inbox.item:..., repo: "acme/api", issue: 2451
}
```

Because the bus spans nodes, this is the *same* call an edge user's UI would make — the
workflow is just another MCP caller. The workflow stays **stateless** (§ core principle 4):
the item it is working on lives in the inbox, the conversation lives in the agent's job
state, nothing durable lives in the workflow instance. Kill it mid-flight and another
instance resumes from the inbox/job records.

### Step 3 — The AI reads context (capability-gated, workspace-first)

The agent gathers context, but every read passes a host-mediated capability check
(§ core principle 5), scoped to workspace `acme` and to what *this request* was granted:

- **Docs/skills** (§6.12) — loads `coding-skills/triage-rubric` and the repo conventions
  doc, *only because* the workspace granted those skills to this agent.
- **Channel history** (§6.2) — reads recent `#backend` discussion for prior decisions.
- **Repository context** — via a granted MCP tool (e.g. a `repo-read` tool exposed by
  `github-bridge` or a code-search extension). The agent never touches the filesystem or
  network directly; it calls tools (§ core principle 7).
- **Related inbox items** — prior issues tagged `repo:acme/api` for duplicate detection.

The agent sees exactly the docs, channels, secrets, tools, and extensions granted to the
workspace and the request — nothing cross-workspace, even though it runs centrally (§6.5).

### Step 4 — The AI writes a scope doc and shares it with the team

The agent produces a **scope doc** as a first-class workspace document (§6.12) and shares
it with team `backend`:

```
doc.create {
  title: "Scope: issue #2451 — fix token refresh race",
  body:  "<problem / proposed change / affected files / risks / test plan>",
  tags:  [ repo:acme/api, issue:2451, kind:scope-doc ],
  share: { team: backend, channel: #issue-2451 }
}
```

Sharing is just bucket/record permissions (§6.12) plus a tag — the doc is private until
shared, then visible to the team and linked into the channel. The agent also posts a short
summary message into `#issue-2451` so humans have a thread.

### Step 5 — The workflow creates an approval inbox item

Reasoning is cheap; **acting on a repo is not**, so the workflow inserts a human gate. It
writes another inbox item — same shape, different intent:

```
inbox.item {
  source:  "coding-workflow",
  type:    "approval.request",
  payload: { scope_doc: doc:..., issue: 2451, proposed_action: "start coding session" },
  tags:    [ needs:approval, repo:acme/api, issue:2451, route:team:reviewers ],
  read:    false
}
```

This appears in the inbox of every member of team `reviewers` (and can be `@`-mentioned in
the channel). Approval is not a core concept — it is *an inbox item with a `needs:approval`
tag and a resolution action*. The workflow waits on its resolution.

> *Scope finding:* inbox items need a lightweight **resolution/action** facet (approve /
> reject / defer, with actor + timestamp) so "approval" is expressible without a bespoke
> table. This is the `inbox-outbox` scope's job to nail down.

### Step 6 — On approval, the workflow starts a remote coding session as a durable job

A reviewer approves (a UI action that resolves the inbox item). The workflow reacts by
creating a **job** (§6.9) — a durable, resumable remote workflow session:

```
job.create {
  kind:    "coding-session",
  input:   { scope_doc: doc:..., repo: "acme/api", issue: 2451, branch: "fix/2451" },
  channel: #issue-2451,        // where to stream progress
  caps:    [ granted MCP tools + skills for this session ],
  state:   { ... resumable session state ... }
}
```

The job is a SurrealDB record (§6.9); a worker on the hub claims it atomically. Crucially,
the job **owns the session**: it survives the edge user disconnecting, the workflow
instance being recycled, or the hub restarting. The approving human can close their laptop;
the session continues on the cloud hub. This is the core reason the agent is hosted
centrally (§6.15).

### Step 7 — The agent runs the session, posts progress, uses granted MCP tools

The job drives the `coding-agent` through the work. As it goes it:

- **Streams progress to the channel** `#issue-2451` (§6.2) — "cloned, running tests,
  drafting patch…" — so humans watch in real time. These are fire-and-forget bus messages.
- **Uses granted MCP tools** — edit, run-tests, repo-search, etc., each capability-checked.
- **Loads skills** as needed (the coding rubric, the PR-description template), each only if
  granted.
- **Checkpoints state** into the job record so a crash resumes mid-session, not from zero.

Note the two different message classes (§6.2): progress chatter is fire-and-forget on the
bus; anything that *must* land in the outside world is **not** sent from here — it goes
through the outbox (next step).

### Step 8 — Every external effect goes through the outbox

The agent never calls GitHub (or email, or a webhook) directly from inside the session.
Each external effect is written as an **outbox row in the same transaction** as the domain
change (the transactional-outbox pattern, §6.10), then a relay publishes it durably:

```
outbox.row {
  target:  "github",
  action:  "create_pr",
  payload: { repo: "acme/api", head: "fix/2451", base: "main", title, body },
  tags:    [ issue:2451 ],
  status:  "pending"
}
```

`github-bridge` (the same adapter from Step 1, now acting outbound) consumes pending rows
and performs them, marking each delivered or failed-with-retry. The same path carries
GitHub comments, reviewer notifications, and the **sync publish** that pushes the new
docs/job-result up so every edge cache converges (§6.8). If GitHub is down, the row waits
and retries — the durability backstop, not best-effort pub/sub.

> *Scope finding:* outbox needs per-target relays with idempotency keys and retry/backoff,
> and the relay set must itself be extension-provided (so new targets don't touch the core).

### Step 9 — Results are saved back across the surface

When the session completes, the results land in the ordinary workspace surface — no special
"result store":

- **Docs** (§6.12) — the final implementation notes / changelog, shared with `backend`.
- **Messages** (§6.2) — a completion summary in `#issue-2451`, `@`-mentioning the reviewer.
- **GitHub updates** (via outbox) — the opened PR, a comment linking the scope doc.
- **Follow-up inbox items** — e.g. `needs:review` on the new PR for team `reviewers`, or
  `needs:triage` if the agent discovered a related bug. The loop can re-enter itself.

The job is marked complete; its transcript is retained per the job's retention policy. Edge
nodes pull the new shared docs and messages on next sync and the whole workspace — online
or offline — sees the same outcome.

---

## 4. Why this belongs in extensions, not the core

Every numbered step above is a **composition**, not a new kernel feature:

| The product wants… | …is just this core primitive |
|---|---|
| "A GitHub issue shows up to work on" | an inbox item with `source:github, needs:triage` |
| "Ask the AI to triage" | an MCP call to a workspace-scoped agent actor |
| "The AI knows our conventions" | granted docs + skills (capability-checked reads) |
| "Write up a plan and share it" | a doc with team/channel share permissions |
| "A human signs off first" | an inbox item with `needs:approval` + a resolution |
| "Do the work without losing it if I disconnect" | a durable job on the hub |
| "Watch it happen" | progress messages on a channel (bus subject) |
| "Actually open the PR / notify people" | outbox rows + a per-target relay |
| "Everyone, including offline users, sees the result" | docs/messages + sync via outbox |

The core stays true to its principles: symmetric nodes, one datastore, state-vs-motion,
stateless extensions, capability-first, workspace-as-the-hard-wall, MCP-as-the-contract.
Swap `github-bridge` for `jira-bridge`, swap `coding-agent` for `review-agent` or
`release-agent`, swap the skills — and you have a different product on the **same** core.
That substitutability is the test the architecture has to pass.

---

## 5. Findings this example surfaces for the coding scope

Collected from the inline notes above; each is an existing open decision (core scope §13)
that this walkthrough makes concrete:

1. **Inbox item facets** — `needs:triage` / `needs:approval` and a resolution
   (approve/reject/defer + actor + ts) must be expressible generically, not per-product.
2. **Outbox relays as extensions** — per-target delivery (GitHub, email, sync) with
   idempotency keys and retry/backoff, registerable without touching the core.
3. **Remote workflow session schema** — job state shape, checkpointing, channel/inbox
   progress hooks, approval checkpoints, cancellation, retry — the resumability contract.
4. **Central-agent capability scoping** — proving a hub-hosted agent's reads stay
   workspace-first and per-request, with audit logging of every tool call and doc/skill load.
5. **Skill/doc grant model** — "the agent may load skill X only if granted" needs a clear
   grant primitive and revocation story.
6. **Idempotent webhook ingress** — replay-safe inbox writes for retried deliveries.
7. **Edge-invoke-central parity** — an edge user's MCP call and the workflow's MCP call to
   the agent must be the identical routed path (§6.5), so offline/online behavior is uniform.

---

## 6. Related reading

- Core stack scope — `../../README.md` (§6.5 MCP, §6.9 jobs, §6.10 inbox/outbox,
  §6.12 files/docs/skills, §6.15 shared AI agents, §7 tenancy).
- `0001-platform-north-star.md` — why the platform exists and where it's going.
- Per-area scope docs under `../scope/` — `workspace`, `channels`, `inbox-outbox`, `jobs`, `mcp`,
  `auth-caps`, `files`, `ai-plane` — the home for the findings in §5.
