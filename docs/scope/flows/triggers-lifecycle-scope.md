# Flows scope — triggers & lifecycle (what starts a flow, and where)

Status: scope (the ask). Promotes to `public/flows/` once shipped. A sub-doc of the flows spine
([`flows-scope.md`](./flows-scope.md)) — read that first; it owns the canonical **Decisions (v1)**
this doc references by number.

This doc owns the **entry edge** of a flow: the **five trigger kinds** that start a run, the
**enable/disable + start-on-boot** switch, the **two lifecycle passes** — a `react_to_flows_cron`
clock-scan and a `reconcile_flows` state-convergence loop that elects an owner and arms/disarms a
flow's sources — and the **placement** that decides *which node* a flow runs on. It does not own the run
engine itself (→ [`flow-run-scope.md`](./flow-run-scope.md)), the source-node bridge
(→ [`extension-nodes-scope.md`](./extension-nodes-scope.md), Decision 2), or the dashboard write-in
UX (→ [`dashboard-binding-scope.md`](./dashboard-binding-scope.md)).

## Goals

- Define the **five trigger kinds** as the entry nodes of a flow: `manual | cron | event | inject
  | boot`, each with a clear firing path that ends in **one `flow-run` job enqueue**.
- A `react_to_flows_cron` **durable clock-scan** for the `cron` kind, modelled exactly on the shipped
  `react_to_reminders` reactor (`../reminders/reminders-scope.md`): same altitude as
  `react_to_approvals`/`relay_outbox`, deterministic firing id, fire-once-then-skip, injected clock.
- An `flows.enable {id, enabled, start_on_boot}` switch (one cap) over a durable flow, and a separate
  `reconcile_flows` **state-convergence loop** that **elects an owner** ([Decision 10](./flows-scope.md)),
  starts enabled + `start_on_boot` flows, and **arms** their source nodes when the node comes up —
  **disarming** them on disable so no live socket leaks. Two responsibilities, two files (FILE-LAYOUT):
  the clock-scan and the reconciler never share a function.
- A `placement` (`either | cloud-only | local-only`) matched **as data** against the node's role by
  the reconciler — config, never an `if cloud {…}` branch (rule 1).

## Non-goals

- The run engine, version-pinning, pause/edit/resume — that is `flow-run-scope.md` (Decision 1). This
  doc only *enqueues* the run; what happens after is the run engine's.
- The source-node `arm`/`disarm` mechanism and the `ingest.write → series` bridge — owned by
  `extension-nodes-scope.md` (Decision 2). This doc *calls* arm/disarm; it doesn't define them.
- A **cross-node auto-placement scheduler** that chooses the best-fit node for an `either` flow.
  Placement here is the **explicit** label honoured against the local role; role-aware scheduling is
  an explicit **`node-roles` deferral** (`../node-roles/node-roles-scope.md` non-goals), not this doc.
- Natural-language or one-off snooze scheduling — `cron` stores a structured 5-field spec, same as a
  reminder; the React cron-builder UX rides the reminders work, not re-litigated here.

## Intent / approach

A flow's lifecycle is the **reminder pattern raised one level**: a reminder is a schedule that fires
**one action**; a flow is a schedule (or event/manual/boot signal) that fires **one run** of a DAG.
So the machinery is identical and already proven — a **durable clock-scan** (`react_to_flows_cron`), a
**subscription** (`event`), and a **state-convergence reconciler** (`reconcile_flows`: owner election +
`boot` + arm/disarm) — each a stateless function over a durable set, never a long-lived in-process timer
(rule 4). The clock-scan and the reconciler are **two passes in two files**: one fires due schedules,
the other converges the directory to the armed/owned state. The five kinds are just the entry node of the graph
(`Trigger(...)` in the spine's node model); firing any of them means **enqueue one `flow-run` job**,
which is where durability/resume/version-pinning live (Decision 1).

### The five trigger kinds

1. **`manual`** — fired by `flows.run {id, params}` (a user click or a tool/agent call). Pins the
   current `flow.version` into the run (Decision 1) and enqueues the job. No reactor; the call *is*
   the firing.

2. **`cron`** — a `react_to_flows_cron` durable scan finds every `enabled` flow whose `cron` trigger is
   `due` and enqueues **one `flow-run` per firing**. Modelled on the shipped `react_to_reminders`
   (`../reminders/reminders-scope.md`):
   - **Same altitude** as `react_to_approvals` / `relay_outbox` — a stateless scan on the S6 reactor
     cadence, in **its own file `react_to_flows_cron`** (FILE-LAYOUT), *not* folded into the reminders
     reactor (different record, different effect) **nor** into `reconcile_flows` (a clock-scan, not a
     state-convergence loop).
   - **Deterministic firing id** from `(flow_id, scheduled_ts)` → **idempotent**: one scheduled
     instant maps to exactly one `flow-run` job id, so an at-least-once re-scan never double-fires.
   - **Fire-once-then-skip a missed firing** on catch-up after an outage — advance to the next future
     slot, **no backfill storm** (the reminders decision verbatim; "every minute" can't enqueue a
     thousand runs after a long outage).
   - **Cron is stored as a 5-field spec** (state, not UX); `next_after(after, inclusive=false)` is
     computed on an **injected clock** via `croner` (`find_next_occurrence`), **never wall-clock** —
     so the scan is deterministic under test (testing §3). *Rejected:* a timer wheel (durable state in
     a process, dies on restart, useless on an offline edge — rule 4).

3. **`event`** — subscribes to a **source node's** series (`flow:{ws}:{flow}:{node}`) via
   `series.watch` / `bus.watch`; the host-armed `ingest.write` bridge is **Decision 2** /
   `extension-nodes-scope.md`. The event-trigger carries the **canonical `coalesce` config defined in
   `flow-run-scope.md`** — `coalesce: { strategy: latest | leading | trailing | sample, window_ms }`
   (not a separate vocabulary re-invented here) — so a chatty source (an MQTT topic at 1 kHz) **can't
   storm the run queue**: one run per `window_ms`, not one per packet (the fan-out risk the spine
   flags; contract owned by → `flow-run-scope.md`). *Rejected:* a raw per-node Zenoh subject (forks the
   ingest convention, Decision 2) and unthrottled one-run-per-event (the fan-out footgun).

4. **`inject`** — `flows.inject {id, node, value}` **sets a node's *retained* value** (held in
   `flow_input:{ws}:{flow}:{node}`, read by every run), and **fires a run only when the target node
   is a *firing* trigger** ([Decision 9](./flows-scope.md)). The inject-trigger node carries a
   **mode `fire | retain`**: `fire` starts a run with the value; `retain` updates `flow_input` and
   starts **no** run. The cooler control-loop (a slider's `setpoint`, a switch's `enabled`) is built
   from **retained** inject nodes + a **separate `event` trigger** that drives one-shot runs reading
   the retained values — never a long-lived run advancing per inject. The full widget UX is
   `dashboard-binding-scope.md`; this doc owns the verb, its cap, and the `fire | retain` mode.
   *Rejected ([Decision 9](./flows-scope.md)):* a long-lived "parked" run that advances on each
   inject (fights frontier-runs-to-completion).

5. **`boot`** — fired **once** when an `enabled` + `start_on_boot` flow's owner node comes up, via the
   `reconcile_flows` loop below. The "run something the moment the appliance powers on" trigger.

### Enable/disable, start-on-boot, and the two lifecycle passes

A flow is a **durable** `flow:{ws}:{id}` record (spine). `flows.enable {id, enabled, start_on_boot}`
(one cap `mcp:flows.enable:call`) flips two flags: `enabled=false` means **no trigger fires** (the
`react_to_flows_cron` scan skips it, the `event` subscription is dropped, `boot` won't fire);
`start_on_boot` marks a flow `reconcile_flows` should bring up at node start.

The lifecycle is **two passes in two files** (FILE-LAYOUT — one responsibility per file):

- **`react_to_flows_cron`** — the durable **clock-scan** (kind 2 above): fire due `cron` schedules,
  deterministic firing id `(flow_id, scheduled_ts)`, fire-once-then-skip, injected clock. *Nothing* about
  arming or owners lives here.
- **`reconcile_flows`** — the **state-convergence loop**, at the **same altitude** as the
  native-lifecycle reconciler and `react_to_reminders`. On each pass it re-reads the durable flow
  directory for its workspace (per-workspace, re-read each pass — exactly like the github-workflow driver
  re-reading its config), **elects the single owner** ([Decision 10](./flows-scope.md)), and for each
  `enabled` + **placement-matching** flow it owns it **converges** the source to the armed state. On boot
  it additionally brings up each `start_on_boot` flow:

  - **arms** its source nodes (the `arm` from `extension-nodes-scope.md` — start the source, pass it the
    host-allocated series id + validated config), then
  - fires the **`boot`** trigger once (one `flow-run` enqueue).

On `flows.enable {enabled:false}` (or flow delete) `reconcile_flows` **disarms** the sources (stop the
socket) so **no live socket leaks** when a flow is turned off — see **Teardown** below for the ordered,
guarded sequence. Enable/disable is the *only* lifecycle of the source's socket — sources are armed by
the host, never self-starting (Decision 2).

### Teardown — guarded and ordered ([Decision 13](./flows-scope.md))

`flows.enable {enabled:false}` and `flows.delete` tear a flow down in a **fixed order**, **idempotent**
on re-run:

1. **Disarm the sources first** — `reconcile_flows` stops every armed socket the owner holds (no leak),
   so no further `event` firing can land.
2. **Then cancel in-flight runs** — cancel the runs enqueued from this flow, **or refuse the teardown
   when active runs exist** (caller's choice); a run allowed to finish completes on its **pinned**
   version (Decision 1).
3. **Then drop the cron registration** — remove the `react_to_flows_cron` schedule **and its
   deterministic firing ids**, so a late re-scan can never re-fire a torn-down flow.

Never the reverse: removing the `flow` record out from under an armed socket, a live run, or a pending
firing is exactly the footgun Decision 13 forbids. *Rejected ([Decision 13](./flows-scope.md)):* an
unordered/best-effort teardown that drops the record while a socket or run still references it.

### Placement — config, not a branch

A flow has **exactly one owner node**; `placement ∈ { either | cloud-only | local-only }` is the
**eligible set, not replication** ([Decision 10](./flows-scope.md)). It **reuses the extension
placement enum** (`../node-roles/node-roles-scope.md`, Decision-aligned with node-roles) so flows and
extensions share one vocabulary. The reconciler **elects one owner** from the eligible set and arms the
source / fires runs **once** — matching `placement` **as data** against the node's `role`, with **no
`if cloud {…}`** anywhere (symmetric nodes, rule 1):

- **`local-only`** — reads local hardware through a **native source node**; the owner is **its install
  node** (the appliance the sensor is wired to). A non-portable native binding (`node-roles` target
  axis) makes any other owner meaningless.
- **`cloud-only`** — the owner is **a hub-class role**; on an edge node the flow is simply *not
  scheduled* (no error, no branch — the role data just doesn't match).
- **`either`** — the owner is **the home node recorded on the flow** (a portable, role-agnostic flow);
  it is **not** armed on every node that matches.

This is the symmetric-nodes rule applied to flow scheduling, identical to how the extension loader
already gates placement-vs-role. **Cross-node failover** (re-electing an owner when the home node dies)
is a `node-roles` deferral. *Rejected ([Decision 10](./flows-scope.md)):* arming on **every**
placement-matching node — N broker sockets and N runs per event, the spatial dual of a fan-out storm;
and a `role`-aware auto-placer that picks the best node for an `either` flow — also a `node-roles`
deferral (above), not v1.

## How it fits the core

- **Tenancy / isolation:** the `flow`, its `cron` schedule fields, and every firing are
  `…:{ws}:…`-scoped; the `react_to_flows_cron` scan **and** `reconcile_flows` are **per-workspace**, re-reading a
  durable directory each pass (the github-workflow driver pattern). A ws-B reactor never sees, fires,
  arms, or advances a ws-A flow. **Isolation test mandatory.**
- **Capabilities:** `mcp:flows.enable:call`, `mcp:flows.run:call`, `mcp:flows.inject:call` are
  **admin-or-owner** gated verbs. Firing a run still re-checks **every node's own tool cap** under
  `caller ∩ grant` at run time (no widening — spine; `flow-run-scope.md`): enable/disable doesn't
  bypass the per-node gate. Deny path: `flows.run`/`flows.enable` without the cap → refused, no
  firing, no record change.
- **Placement:** the headline of this doc — `either | cloud-only | local-only` matched as data
  against role (above). Config, never a branch.
- **MCP surface:** **CRUD-shaped lifecycle verbs** — `flows.enable` (the on/off + boot flag write),
  `flows.run` (manual fire), `flows.inject` (set a node's retained value; fires a run **only** for a
  `fire`-mode trigger node — [Decision 9](./flows-scope.md)). All bounded, always-fast single-record
  writes (`flow_input` upsert) / single-job enqueues → **synchronous**, not jobs (the *run* they enqueue
  is the durable job). **Get/list** of flows is the spine's CRUD, not re-defined here. **Live feed:** the
  flow's *run* status is watched on the `flow-run` feed; the **lifecycle** record changes only on
  enable/disable, for which `get`/`list` suffice — no `watch` here. **Batch:** N/A (no bulk
  enable/run caller at v1; a future "enable all" would be a bounded sync batch).
- **Data (SurrealDB):** the schedule + `enabled` + `start_on_boot` + `placement` + `next_attempt_ts`
  live on the `flow` record (state). A firing is a **job enqueue** (`flow-run`), **not** pub/sub —
  state vs motion (rule 3). No new table beyond the spine's records + the cron bookkeeping fields.
- **Bus (Zenoh):** none owned here. The `event` trigger *consumes* a source node's series (motion,
  Decision 2); the `cron`/`boot` paths are durable scans/reconcilers, not bus consumers. A run's
  must-deliver effects go through the **outbox** (the run engine's concern).
- **Sync / authority:** node-local record, workspace-authoritative. An **edge flow keeps firing
  offline** — the `cron` scan fires its own workspace's flows with no network; **cloud effects relay
  on reconnect** through the outbox. Missed `cron` firings during an outage fire **once** on catch-up
  (fire-once-then-skip), the deterministic firing id preventing any double-fire on the reconnect scan.
- **Secrets:** none directly. A node needing a secret has it mediated by the tool/`Target` it calls
  at run time; the trigger stores a flow id + schedule + placement, never secret material.

## Example flow

**An MQTT alarm flow on an appliance, then a daily-rollup on the hub.**

1. On the appliance, a flow's `event` trigger watches an `mqtt` source node. The owner sets
   `flows.enable {id, enabled:true, start_on_boot:true}` and `placement:"local-only"`.
2. **Appliance boots.** `reconcile_flows` re-reads the workspace's flow directory, finds this flow
   `enabled + start_on_boot`, and — because `placement="local-only"` **elects this install node as the
   single owner** ([Decision 10](./flows-scope.md)) — brings it up: **arms** the `mqtt` source (start
   the socket, pass the
   host-allocated series id + validated config), then fires the **`boot`** trigger once → one
   `flow-run` enqueued, pinned to the current version (Decision 1).
3. An MQTT alarm packet arrives. The source's `ingest.write` lands it on
   `flow:{ws}:{flow}:{node}` (Decision 2); the `event` trigger's `series.watch` sees it, the
   `coalesce` window (`flow-run-scope.md`) coalesces a burst into one firing, and **one `flow-run`** is enqueued — the
   alarm flow runs.
4. The owner sets `flows.enable {enabled:false}`. **Teardown runs in order** ([Decision
   13](./flows-scope.md)): `reconcile_flows` **disarms** the `mqtt` source first (socket stopped — no
   leak), then the in-flight run from step 3 is cancelled or allowed to **finish on its pinned version**
   (Decision 1), then the cron registration + its firing ids are dropped. No new trigger fires while
   disabled; teardown is idempotent.
5. Meanwhile, a `cron` **daily-rollup** flow set `placement:"cloud-only"` lives on a hub-class node.
   The hub's `react_to_flows_cron` **scan** finds it `due` at `0 6 * * *` (next-time computed on the
   injected clock via `croner`), derives the firing id from `(flow_id, scheduled_ts)`, enqueues one
   `flow-run`, and advances `next_attempt_ts` to tomorrow. On the appliance (an edge role) this same
   flow is simply **not scheduled** — its `cloud-only` placement doesn't match the role; no branch, no
   error.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md` (real store `mem://` / bus / jobs /
outbox — **no mocks**; the only permitted fake is the MQTT broker behind one extension trait; **injected
clock, never wall-clock**, testing §3):

- **Capability-deny** (required): `flows.enable` without `mcp:flows.enable:call` → refused, no flag
  change; `flows.run` without `mcp:flows.run:call` → refused, no firing. A node whose run-time tool
  grant was revoked still denies at the per-node gate (enable doesn't widen).
- **Workspace-isolation** (required): a ws-B `react_to_flows_cron` scan / `reconcile_flows` pass never
  fires, arms, or advances a ws-A flow; the schedule and firings of ws-A are invisible to ws-B's reactor
  and `list`/`get`. Tested across store + clock-scan + reconciler.
- **Offline/sync** (required): an edge `cron` flow fires while disconnected; on reconnect the missed
  slot fires **once** (fire-once-then-skip) and the deterministic `(flow_id, scheduled_ts)` firing id
  yields **no double-fire** on the reconnect scan.

Key unit/integration cases:

- `cron` `next_after(after, inclusive=false)` on an **injected clock** is deterministic (multi-value
  fields, month/day rollover, logical-clock DST-agnostic).
- **Fire-once-then-skip:** a re-scan before `next_attempt_ts` advances fires nothing twice; a long
  outage produces exactly one catch-up firing, not a backfill storm.
- **`reconcile_flows` owner election + boot:** the loop elects **exactly one owner**
  ([Decision 10](./flows-scope.md)) and starts **only** `enabled + start_on_boot + placement-matching`
  flows on it — a disabled, a non-boot, or a placement-mismatched flow is **not** started, and a flow is
  **never** armed on two nodes at once (no N-socket fan-out).
- **Ordered teardown** ([Decision 13](./flows-scope.md)): `flows.enable {enabled:false}` / `flows.delete`
  **disarms the source first**, then cancels-or-refuses in-flight runs, then drops the cron registration +
  its firing ids; the socket is stopped (no leak), an allowed run **completes on its pinned version**, a
  late re-scan **cannot** re-fire, and a re-run of teardown is a no-op (idempotent).
- An **`either`** flow does **not** start on a role its placement excludes — i.e. a `cloud-only` flow
  on an edge role and a `local-only` flow on a non-install node are both skipped, as data, no branch.
- **`inject` mode** ([Decision 9](./flows-scope.md)): an inject into a `retain` node updates
  `flow_input` and starts **no** run; an inject into a `fire` trigger node starts exactly one — and a
  subsequent event-run reads the retained value.

## Risks & hard problems

- **At-least-once → idempotency.** The `react_to_flows_cron` scan is at-least-once; the deterministic
  `(flow_id, scheduled_ts)` firing id is the only thing standing between "fires once" and "double-runs
  a flow" — the same discipline the relay/approval reactors already depend on. Get it wrong and an
  alarm flow runs twice.
- **Socket lifecycle vs. enable state.** A source armed but never disarmed (a crash between
  `enable=false` and disarm) leaks a live socket; `reconcile_flows` must **converge** the directory to
  the arm/disarm state on every pass (re-read, reconcile), not just react to the edge — same posture as
  the native-lifecycle reconciler, and the reason teardown (Decision 13) disarms before it drops a record.
- **Coalesce under a chatty source.** The `event`-trigger `coalesce` window (the canonical
  `flow-run-scope.md` enum) is the only guard against one-run-per-packet fan-out; an under-tuned
  `window_ms` storms the job queue. The contract is **owned by** `flow-run-scope.md`; this doc must hand
  it a real chatty-source test, not a synthetic tick.
- **Placement drift / owner election on role change.** A node that changes role (edge → hub) must
  re-evaluate which flows it owns on the next `reconcile_flows` pass — placement is the eligible set
  matched each pass as data and the owner re-elected ([Decision 10](./flows-scope.md)), so this is a
  re-read, not a migration, but it must be exercised.

## Related

- [`flows-scope.md`](./flows-scope.md) — the spine; **Decisions 1** (versioned run / in-flight finishes
  on its pinned version), **2** (host-armed source bridge), **9** (`inject` retained-vs-firing), **10**
  (single owner / placement = eligible set), and **13** (ordered teardown) are load-bearing here.
- [`flow-run-scope.md`](./flow-run-scope.md) — the run engine the triggers enqueue into; **owns** the
  `event`-trigger `coalesce: { strategy, window_ms }` contract this doc references.
- [`extension-nodes-scope.md`](./extension-nodes-scope.md) — the source-node `arm`/`disarm` and the
  `ingest.write → series` bridge (Decision 2) this lifecycle drives.
- [`dashboard-binding-scope.md`](./dashboard-binding-scope.md) — the `flows.inject` write-in UX.
- [`../reminders/reminders-scope.md`](../reminders/reminders-scope.md) — the shipped `react_to_reminders`
  durable-scan reactor + `croner` + fire-once-then-skip this `cron` kind is modelled on.
- [`../node-roles/node-roles-scope.md`](../node-roles/node-roles-scope.md) — the `placement` enum and
  the cross-node auto-placement deferral this doc honours as a non-goal.
- README `§3` (rules 1/3/4/5/6), `§6.9` (jobs), `§6.10` (outbox), `§6.13` (gateway SSE / gates).
