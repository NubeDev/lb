# Workflow-driver scope — the background loop that runs the coding workflow

Status: scope (the ask). Promotes to `public/coding-workflow/` with the slice.

> Read with: `coding-workflow-scope.md` (the orchestration verbs), `../inbox-outbox/outbox-scope.md`
> (the relay + reactor it drives), `../node-roles/node-roles-scope.md` (placement = config, §3.1),
> `../../sessions/coding-workflow/close-the-loop-session.md` (the reactor),
> `../extensions/github-webhook-scope.md` (the ingress it shares a node with).

The coding workflow's durable-scan verbs — `react_to_approvals` (auto-start jobs on approval) and
`relay_outbox` (deliver PR/comment effects) — were proven in tests but **nothing ran them in a
process**. The `node` binary ended at the S1 hello demo. This slice ships the **background driver**:
a long-running service that ticks those two verbs per workspace, plus the env-gated wiring that mounts
it (and the webhook ingress) in the binary. It turns "the loop is correct" into "a node runs it".

## Goals

- A **`run_workflow_loop`** service: every `interval`, for each configured workspace, run a **reactor
  pass** then a **relay pass** (reactor first, so a freshly-approved job's PR goes out the same tick).
- A **`drive_once`** single-tick function under it — the testable unit, returning a `Tick` tally.
- **Per-workspace bindings** (`WorkflowBinding { ws, principal, channel }`): the loop services a
  *list* of them, each selecting its own `ws`, so the wall holds structurally (a tick for ws-A can
  neither deliver ws-B's effects nor start ws-B's jobs).
- **Injected clock**: `now` is a closure the caller supplies — the binary passes wall-clock seconds,
  a test passes a counter. No wall-clock inside the crate (testing §3); the binary is the boundary.
- **Env-gated wiring in `node`** (`node/src/github.rs`): mount the webhook front door + the driver
  loop iff configured. Config, not a code branch (§3.1) — absent config = the solo demo.

## Non-goals (this slice)

- **A LIVE-query driver** (instant pickup on a new approval/effect). The durable scan is the source
  of truth; the LIVE push is the latency optimization, deferred (same as the relay/reactor scopes).
- **Multi-relay contention** (two driver processes racing for one workspace). One driver per
  deployment; the atomic-claim primitive is the jobs-queue follow-up.
- **A config file / dynamic workspace set.** The binary configures one workspace from env; reading a
  durable binding set (many workspaces, hot-added) is the next step (the registry already does this
  shape for tenants).
- **Backoff/scheduling beyond what the relay already does** — the driver just supplies `now`; the
  backoff gate + dead-letter live in the outbox relay (already shipped).

## Intent / approach

**The host owns the verbs; this owns the loop.** `relay_outbox` and `react_to_approvals` are stateless
functions over durable sets (host services). The driver adds only the *cadence* — the same split as
`lb-role-gateway` (host owns the MCP pipeline, the gateway owns the HTTP server) and
`lb-role-github-webhook` (host owns `ingest_via_bridge`, the webhook owns the HTTP edge). So the driver
is a thin orchestration loop with **no network deps of its own**: the GitHub HTTP `Target` is
`lb-role-github-target`, supplied behind the host trait. **Rejected:** putting the loop inside
`lb-host` — it would pull a timer + a concrete target into core, and make "run the loop" non-optional.

**A tick never fails the loop.** A per-workspace error (a store blip) is reported to an `on_error`
sink and skipped; the tick still services the other workspaces, and the next tick re-reads the durable
set (the not-yet-delivered effect / not-yet-started approval is still owed — never lost). This is the
stateless-service property the outbox/reactor already guarantee, lifted to the loop.

**Reactor before relay, each tick.** Auto-start approved jobs first (queues their PR effects), then
deliver every due effect — so an approval landed since the last tick produces a PR in *this* tick, not
the next. Both are idempotent, so the ordering is an optimization, not a correctness requirement.

## How it fits the core

- **Tenancy / isolation:** the loop services a *list* of bindings; every `relay_outbox` /
  `react_to_approvals` call selects its binding's `ws`. Mandatory isolation test: a tick whose list is
  only ws-A leaves ws-B's approved job + effects entirely untouched (store + the two verbs).
- **Capabilities:** the driver acts as each binding's **service principal**, which holds the workflow
  caps (`mcp:workflow.start_job:call` for the reactor). The reactor re-checks that gate, so an
  under-granted service identity cannot start jobs (the deny path is already tested at the reactor;
  the driver inherits it). The relay needs no cap (a host service over the durable set).
- **Placement:** *either node*, by config (symmetric nodes). The driver runs where the target is
  reachable (default: the hub). Mounting it is which node's binary sets the env — not a code branch.
- **MCP surface:** none. The driver consumes the host verbs; it exposes no tool (like the relay, it is
  a durability mechanism, not a tool surface).
- **Data / Bus:** drives the outbox (state) + streams job progress to a channel (motion) — both
  through the existing verbs. The driver itself holds no state (stateless, §3.4).
- **Sync / authority:** N/A new — the driver reads the same `(table,id)` records the sync path covers.

## Testing plan

Mandatory categories (testing §2):

- **Workspace-isolation** (§2.2): a tick over only ws-A never touches ws-B's job/effects.
- **Capability-deny** (§2.1): inherited from the reactor's own deny test (an under-granted service
  principal starts no job) — the driver does not weaken it.
- Unit/integration (real embedded SurrealDB + in-proc Zenoh; the GitHub sink is the only stub):
  - one tick auto-starts an approved job AND delivers its PR (the headline);
  - a second tick is a no-op (loop-level idempotency — one job, one PR);
  - the injected clock advances across ticks (no wall-clock in the crate; `now` threaded to both verbs).

## Open questions

- ~~**Dynamic workspace set.**~~ **RESOLVED (S7):** a durable **workflow directory**
  (`register_workspace` / `deregister_workspace` / `enabled_workspaces` in a reserved namespace) the
  driver re-reads each tick — a workspace is onboarded/retired without a restart. The binding list is no
  longer fixed at boot; `run_directory_loop` / `drive_directory_once` build bindings from the directory.
  See `../../sessions/coding-workflow/dynamic-directory-session.md`. *Still open here:* the webhook
  **tenant** directory (paired with `lb-secrets` for per-tenant secrets), an admin/MCP surface to drive
  the register verbs, and GC of long-disabled rows.
- **LIVE-query driver.** Replace the poll tick with a LIVE subscription on the outbox + resolution
  tables for instant pickup. (Open — deferred with the relay/reactor LIVE follow-up.)
- **Multi-driver contention.** Two drivers on one workspace need the outbox atomic-claim. (Open —
  one driver per deployment now.)
- **Backpressure / tick budget.** A workspace with thousands of due effects could starve others within
  a tick; a per-tick cap + round-robin is a follow-up if it bites. (Open.)

## Related

- `coding-workflow-scope.md` — the verbs the driver runs.
- `../inbox-outbox/outbox-scope.md` — the relay + the reactor (the two passes).
- `../node-roles/node-roles-scope.md` — placement-by-config; the binary's role-aware wiring.
- `../../sessions/coding-workflow/workflow-driver-session.md` — the build log.
