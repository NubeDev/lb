# Coding workflow — the worked example end to end (session)

- Date: 2026-06-26
- Scope: ../../scope/coding-workflow/coding-workflow-scope.md (NEW this session) +
  ../../scope/inbox-outbox/outbox-scope.md (NEW this session) +
  ../../scope/jobs/jobs-scope.md + ../../scope/agent/agent-scope.md +
  ../../scope/inbox-outbox/inbox-outbox-scope.md + ../../scope/files/files-scope.md
- Stage: S6 — coding workflow extension (STAGES.md)
- Status: done

## Goal

Build S6 as a **vertical slice** composing the S5 primitives (agent + jobs) with the new
must-deliver **outbox**: a GitHub issue → inbox `needs:triage` → the central agent triages and
drafts a shared scope doc → a `needs:approval` inbox item that **genuinely gates** a durable coding
job → progress streams to a channel → **every external effect goes through the transactional outbox
with retry**.

**Exit gate (S6), restated:** the full flow runs; the approval genuinely gates the job; every
external effect (PR, comment, notify, sync) goes through the outbox with retry.

## What changed

### Scope authored first (two scopes were only stubs / missing)

Per HOW-TO-CODE §1 + SCOPE-WRITTING, the missing contracts were written before any code:
- **`scope/inbox-outbox/outbox-scope.md`** (NEW) — the transactional must-deliver outbox (the S6
  driver). The load-bearing decision: the domain change AND the outbox row are written in **one
  SurrealDB transaction**; a relay delivers `pending` at-least-once; the receiver dedups on
  `idempotency_key`. Rejected reusing the job queue for effects (different lifecycle); rejected the
  relay calling a concrete HTTP client (un-testable, un-swappable) in favour of a `Target` trait.
- **`scope/coding-workflow/coding-workflow-scope.md`** (NEW) — the orchestrator slice. The approval
  gate is **data, not a primitive** (an inbox item + a resolution facet); `start_coding_job` refuses
  unless `Approved` and that refusal *is* the gate. The workflow/bridge are **host services** at S6
  (same reasoning as the agent), packaged as wasm extensions only at S7.

### store — the transactional seam (`write_tx`)

`lb_store::write_tx(store, ws, change: Upsert, effect: Upsert)` issues one `BEGIN…COMMIT
TRANSACTION` around both upserts. This is the entire transactional-outbox mechanism: both commit or
neither does — no window where the change is durable but the effect is lost, nor vice-versa. Same
`data`-envelope as `write`, so the read path is unchanged.

### store — the new `lb-outbox` crate (must-deliver effects, no auth)

Mirrors `lb-inbox`/`lb-jobs`: the `Effect` record + raw verbs, workspace-namespaced, **no
authorization** (the host workflow service is the chokepoint). One verb per file:
- `enqueue` — the transactional write (over `write_tx`): the domain change + the effect, atomic.
- `pending` — scan `status ∈ {pending, failed}` (both schedulable) — the relay's **durable
  backstop**, ordered by injected `ts`.
- `mark_delivered` / `mark_failed` — record a delivery attempt; `failed` stays schedulable (retry),
  `delivered` stops re-delivery. Each counts `attempts`.

### inbox — the resolution facet (the approval gate's subject)

`lb_inbox::Resolution` (`{item_id, decision: approve|reject|defer, actor, ts}`) + `resolve` /
`resolution` verbs — a small sibling record keyed by the item id. `Item` stays stable (a non-goal to
change it). This is the vision §5 "inbox item resolution" finding, expressed generically.

### host — the `workflow` service (the orchestration + the gate), beside `agent/`/`channel/`/`assets/`

One responsibility per file (FILE-LAYOUT §3):
- **`ingest_issue`** — github-bridge writes the inbox `needs:triage` item (idempotent on the issue
  id — replay-safe webhook ingress).
- **`triage`** — drives the S5 agent over the **same `invoke` path** an edge user uses (edge-invoke
  parity), then `put_doc` + `share_doc` the draft (as the caller) and posts a channel summary.
- **`request_approval`** — writes the `needs:approval` inbox item routed to a team.
- **`resolve_approval`** — writes the resolution; `actor` forced to the principal's sub (no forging).
- **`start_coding_job`** — **THE GATE**: refuses (`AwaitingApproval`) unless the approval is
  `Approved`, creating nothing; on approval creates the durable job, streams progress, and routes the
  PR through the outbox.
- **`emit_effect`** — the transactional must-deliver write: append a job step AND enqueue the effect
  in one transaction (over `lb_outbox::enqueue`).
- **`relay_outbox`** — deliver `pending` at-least-once through a `Target`, marking each outcome; a
  failed delivery retries next pass.
- **`target`** — the host-owned `Target` trait (the delivery seam, like the agent's `ModelAccess`).
- **`tool`** — the `workflow.*` MCP bridge (ingest / request_approval / resolve_approval / start_job;
  `triage` is not bridged — it needs a `ModelAccess`, like the agent's `invoke`).

### MCP — the workflow over the one contract

`workflow.<verb>` reached through `lb_mcp::authorize_tool` (the same host-native bridge gate as
`assets.*`/`agent.invoke`), workspace-first, then the verb's own gate (`start_job` re-checks the
approval). Two independent surfaces (the MCP grant + the approval gate), both enforced.

### UI — minimal WorkflowView + api client (mirrors the verbs)

`lib/workflow/{workflow.types,workflow.api}.ts` (one call per export) → `lib/ipc/workflow.fake.ts`
(a faithful in-memory node: the capability gate + the approval gate, so the UI's allow/deny/gated
paths are exercised) → `features/workflow/` (`useWorkflow` hook + `WorkflowView` + barrel). Wired
into the `fake.ts` dispatcher (workflow commands before assets/channel). No change to existing
surfaces.

## Decisions & alternatives

- **Transactional outbox = the domain change + the effect in ONE transaction** — the whole point of
  S6. `write_tx` is one small `BEGIN…COMMIT` verb; `enqueue` is the only path. **Rejected:** writing
  the effect after the change (best-effort), which reopens the orphaned-effect / lost-effect window
  §6.10 exists to close.
- **The approval gate is data (resolution facet), not a primitive** — an inbox item + a `Resolution`
  sibling; the job-start verb reads it. **Rejected:** a dedicated `approval` table / a workflow state
  machine — it would duplicate the inbox's normalized shape + routing/unread machinery.
- **At-least-once + receiver dedup, not exactly-once** — the relay guarantees delivery; correctness
  rests on a stable `idempotency_key` the receiver honors. **Rejected:** trying to make the relay
  exactly-once (impossible across a crash boundary without a dedup key anyway).
- **The relay delivers to a `Target` trait (host-owned), not a concrete client** — mirrors the
  agent's `ModelAccess` seam: testable deterministically, swappable, extension-providable.
  **Rejected:** the relay calling GitHub HTTP directly (un-testable without network, un-swappable).
- **`pending` = `pending ∪ failed`** — a failed effect is still owed, so it must re-appear for the
  relay. Kept distinct from `pending` only for audit (`attempts`). **Rejected:** marking a failed
  effect back to `pending` (loses the "has been tried" signal).
- **The workflow is a host service, not a wasm extension (at S6)** — the orchestration drives
  `caps::check`, the agent loop, jobs, and the transactional outbox — all host-internal seams (same
  reasoning as the agent being a host service, agent scope). **Rejected:** a wasm extension that
  round-trips every one of those back through the host. Packaging as a registry artifact is S7.
- **Effects go through the outbox; progress chatter rides the bus** — the two message classes (§6.2)
  kept distinct: the must-deliver PR is an `Effect` row; "job started" is a fire-and-forget channel
  post. **Rejected:** streaming the PR effect on the bus (a disconnect could drop a must-deliver).

## Tests

Mandatory categories (testing §2) — the S6 gate, not extras. Determinism held: all `ts`/ids
injected; a unique workspace id per node-booting test; multi-thread flavor on every node test (Zenoh
peer); the model provider + the GitHub `Target` are the **only** externals mocked (real embedded
SurrealDB + in-proc Zenoh + real wasm everywhere else).

New this slice:

- **outbox `outbox_test` (4)** — the transactional enqueue is atomic (change + effect both land); a
  failed delivery stays schedulable and re-delivers; re-enqueue is idempotent on the effect id;
  **store-layer ws isolation** (a ws-B scan/mark can't touch a ws-A effect).
- **inbox `resolution_test` (4)** — a decision persists + reads back; re-resolving upserts (last wins
  — a deferred item can later approve); unresolved reads `None`; **ws isolation** (a leaked approval
  would defeat the gate).
- **host `workflow_test` (3)** — THE EXIT GATE: issue → triage + shared scope doc → the approval
  **genuinely gates** (no job before approval; refused with `AwaitingApproval`) → on approval the job
  starts, streams progress, queues the PR through the outbox, and the relay delivers it once;
  **capability-deny** (each verb refused without its grant); a **rejected** approval never starts it.
- **host `workflow_isolation_test` (2)** — **MANDATORY ws-isolation** across store + MCP: a ws-B
  caller sees none of ws-A's issues/approvals/job/effects and a ws-B relay delivers nothing of
  ws-A's; the `workflow.*` MCP bridge denies (no grant) and isolates (a ws-B principal can't drive a
  ws-A call, workspace-first).
- **host `workflow_offline_test` (3)** — **MANDATORY offline/sync** (the S6 headline): an effect
  **survives an outage** and is delivered at-least-once on retry (never lost); a **duplicate
  delivery is a no-op** on the receiver (dedup on `idempotency_key` — never double-sent); the
  **enqueue is atomic** (the job step + the effect commit together).
- **ui `WorkflowView.test.tsx` (4, Vitest)** — approving lets the job start + queues the PR through
  the outbox; an **unapproved** start is refused ("awaiting approval"); a user **without the grant**
  is denied; a **rejected** approval still refuses — driving the real `workflow.api` → `invoke` →
  fake path.

### Green output

Run per-binary / bounded parallelism — node-booting tests make a single `cargo test --workspace`
OOM (debugging/bus/cargo-test-workspace-ooms-with-many-peers.md).

```
# Rust — light crates (real embedded SurrealDB)
$ cargo test -p lb-outbox                → 4 passed   # NEW: transactional enqueue + retry + ws iso
$ cargo test -p lb-inbox                 → 8 passed   # +4 resolution facet (was 4)
$ cargo test -p lb-jobs                  → 4 passed
$ cargo test -p lb-role-ai-gateway       → 3 passed
$ cargo test -p lb-caps                  → 22 passed
  auth 4  bus 2  ext-loader 2  store 5  assets 8  sdk 3  → light total: 65 passed

# Rust — host integration (real wasm + real SurrealDB + Zenoh), per-binary
$ cargo test -p lb-host --test spine_test               → 4 passed
$ cargo test -p lb-host --test messaging_test           → 3 passed
$ cargo test -p lb-host --test messaging_deny_test      → 3 passed
$ cargo test -p lb-host --test messaging_isolation_test → 2 passed
$ cargo test -p lb-host --test presence_test            → 2 passed
$ cargo test -p lb-host --test hot_reload_test          → 2 passed
$ cargo test -p lb-host --test cross_node_routing_test  → 3 passed
$ cargo test -p lb-host --test offline_sync_test        → 3 passed
$ cargo test -p lb-host --test assets_doc_test          → 6 passed
$ cargo test -p lb-host --test assets_skill_test        → 3 passed
$ cargo test -p lb-host --test assets_isolation_test    → 3 passed
$ cargo test -p lb-host --test assets_mcp_test          → 4 passed
$ cargo test -p lb-host --test install_record_test      → 2 passed
$ cargo test -p lb-host --test agent_test               → 4 passed
$ cargo test -p lb-host --test agent_isolation_test     → 2 passed
$ cargo test -p lb-host --test agent_offline_test       → 2 passed
$ cargo test -p lb-host --test agent_routed_test        → 3 passed
$ cargo test -p lb-host --test workflow_test            → 3 passed  # NEW: EXIT GATE + deny + gate
$ cargo test -p lb-host --test workflow_isolation_test  → 2 passed  # NEW: MANDATORY ws-isolation
$ cargo test -p lb-host --test workflow_offline_test    → 3 passed  # NEW: MANDATORY offline/outbox
   host total: 59 passed   (was 51 at S5; +8 workflow)

   RUST TOTAL: 124 passed, 0 failed   (was 105 at S5; +4 outbox +4 inbox +8 host +3 sdk-now-counted)

# Tauri shell command layer (headless) — unchanged, still green
$ cd ui/src-tauri && cargo test          → 2 passed

# UI (Vitest) + type-check + bundle
$ cd ui && pnpm test                     → 18 passed (6 files)   # +4: WorkflowView approval gate
  ChannelView 3  channel.api 3  useChannel 2  DocView 3  AgentView 3  WorkflowView 4
$ pnpm build                             → tsc --noEmit clean; vite build ✓

# Formatting + file size
$ cargo fmt --all --check                → FMT OK
$ bash rust/scripts/check-file-size.sh   → all source files within 400 lines (211 checked)
```

## Debugging

None — no non-trivial bug surfaced. (See "Dead ends" for the one fixture gap, which was a missing
test grant, not a code defect, so it gets no `debugging/` entry per debugging-scope §3.)

## Public / scope updates

- Promoted to `public/`: `coding-workflow` (NEW — the worked example) and the **outbox** section of
  `inbox-outbox` (the transactional must-deliver path). Refreshed `public/SCOPE.md` with the S6 row.
- Resolved/refreshed open questions: `inbox-outbox` (the outbox is built — "what shipped" added; the
  inbox `Item` stayed stable, the resolution facet landed as a sibling); `jobs` (the outbox/job-queue
  split decided — separate tables); `coding-workflow` + `outbox` open questions recorded as
  follow-ups (backoff/dead-letter, multi-relay atomic claim, FIFO ordering, target registration, the
  LIVE-query relay reactor, wasm packaging).

## Dead ends / surprises

- The exit-gate test first failed `Denied` at `triage`: posting the triage **summary** to the
  channel needs a `bus:chan/*:pub` grant the test caller lacked. Not a code bug — the channel
  chokepoint (capability-first) was doing its job; the fixture just needed the pub cap (the workflow
  posts motion). Fixed by granting `bus:chan/*:pub` in the workflow test callers. Worth noting: this
  confirms the workflow doesn't get a privileged back channel — it posts as the caller, gated like
  any poster.

## Follow-ups

- **Real target adapters** behind the `Target` trait (GitHub HTTP, email, the sync publish) — the
  in-test target is the only stub; the trait seam is ready (S7).
- **Backoff + dead-letter** for perpetually-failing effects, and the **multi-relay atomic claim**
  (`UPDATE … WHERE status='pending' RETURN BEFORE`) — deferred with the jobs queue (no pressure yet).
- **The LIVE-query relay reactor** (instant pickup) + a **resolution reactor** that auto-starts the
  job on approval — S6 uses durable scans + an explicit `start_job` call (the latency optimization is
  layered on later).
- **Webhook ingress server** for `github-bridge` (real HTTP) + **packaging** the workflow/bridge as
  installed wasm artifacts — S7 registry.
- **Idempotency-key derivation helper** (S6 takes it as an explicit caller arg).
- STATUS.md updated? **Yes** — coding-workflow slice marked `shipped`; S6 exit gate met.
</content>
