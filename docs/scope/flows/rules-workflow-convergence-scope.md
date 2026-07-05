# Flows scope — rules + workflow convergence, and the webhook source node

Status: scope (the ask). Promotes to `public/flows/flows.md` once shipped.

Flows is the spine. Two older subsystems that grew up beside it — the **rules engine**
(a Rhai cage + saved rules) and the **coding "workflow"** (the S6 issue→triage→approval→PR
orchestrator) — are converged INTO flows as generic nodes. A rule becomes a flow node
(inline Rhai, or run-a-saved-rule); the workflow's *generic machinery* (the approval gate,
the transactional-outbox delivery) becomes flow nodes, and its *GitHub/coding-specific*
parts are **hard-deleted** (never used in production, no data to preserve). We also add the
one remaining inbound flow surface: a generic **webhook source node** that wraps the
already-shipped webhook backend. After this, there is one engine to reason about, one place
new automation is authored, and **zero** provider names in any core crate (rule 10).

## Goals

- **Rules ARE flow nodes.** Ship the missing `rules.eval` host verb (same cage as
  `rules.run`) that takes the flow message envelope and returns `{output, findings, log}`,
  and wire the already-present `rhai` built-in node to it. Add a sibling node that runs a
  **saved** rule by name + params.
- **Engine guards.** A per-flow **concurrency policy** (`skip | queue | restart`) enforced
  at fire time in both the cron reactor and `flows.run`; a per-node **`timeout_ms`** enforced
  around dispatch (a slow node settles `err:"timeout"`); the Rhai cage's wall-clock deadline
  exposed as a node config knob.
- **Workflow converges.** Keep the generic pieces as flow nodes — an **approval-gate node**
  (parks the run on the existing Park/resume machinery until a human resolves) and an
  **outbox-sink node** (durable OUTBOUND delivery via the existing outbox + `relay_outbox`
  retry/backoff/dead-letter). **Hard-delete** every GitHub/coding-specific piece with no
  migration.
- **Webhook source node.** A generic built-in `webhook` **source** node whose config is
  `{webhook_id}` (a picker over `webhook.list`); arming subscribes to the core webhook's
  series `webhook:{ws}:{id}` and fires a run per hit. The node owns no endpoint/credential —
  it wraps the core service. This is the ONLY flow-facing inbound surface.

## Non-goals

- **No Slack node, no GitHub node, no provider name in a core crate.** Not now, not as a
  "built-in". Provider integrations are extensions reached through the generic seams
  (`<ext>.<tool>` dispatch, the outbox `Target`, `ext.list`).
- **No migration of the coding workflow.** It is deleted, not deprecated — confirmed unused
  in production, no records to preserve. (`docs/scope/coding-workflow/*` is marked retired,
  mirroring `chains-retirement-scope.md`.)
- **The webhook backend + admin wizard UI are out of scope** — already shipped
  (`webhooks-scope.md`); we only add the flow **source node** over it.
- **No new inbound machinery.** Inbound = webhooks (a source node). Outbound = the outbox
  sink. The two are named apart and never conflated.
- Chaining/parallel joins need nothing new — they are the existing DAG `needs` edges.

## Intent / approach

The convergence is a *deletion + rewiring*, not a rewrite, because the flows engine already
owns every primitive the workflow hand-rolled:

- **Rule as node:** the `rhai` descriptor already exists in `builtins/core.rs` and already
  declares `tool: "rules.eval"` — but there is no `rules.eval` verb, and `execute_node/core::rhai`
  currently dispatches `rules.run` directly. We ship `rules.eval` as a thin sibling of
  `rules.run` (same `RuleEngine`, same cage, same seams) that speaks the **flow envelope**
  (`{payload, topic}` in → `{output, findings, log}` out) so findings render on the canvas,
  and repoint the node at it. A new `rule` node runs a *saved* rule by `name` + `params`
  (dispatches `rules.eval {rule_id, params}`). We reject generalizing the cage — the cage is
  the security boundary and stays exactly as-is; only the entry verb is new.
- **Approval gate as node:** the run already parks (`Dispatched::Park`) on a durable timer
  (the `delay` node) and resumes via `flows.resume`. The gate node parks the same way but on
  an **external resolution**: on first arrival it writes a `needs:approval` inbox item keyed
  by `(run_id, node_id)` and parks; a small **flow-approval reactor** (twin of the existing
  generic `approval_reactor`) resumes the run when the item resolves `Approved`, or cancels
  it on `Rejected`. On re-drive the node reads the resolution and settles through. We reject
  reusing the coding-workflow's `request_approval` (it is bolted to `PrSpec`) — the generic
  `inbox` verbs already carry the exact shape with no provider coupling.
- **Outbox sink as node:** the `sink` node already has a `target: "outbox"` leg (stages a
  transactional effect). What was missing is a *driver*: `relay_outbox` (the retry/backoff/
  dead-letter delivery through a `Target`) was driven ONLY by the github-workflow loop we are
  deleting. `relay_outbox` + the `Target` trait are github-free and already depended on by
  the reminders reactor test, the approval-release test, and the ROS extension (through the
  `lb_host::{Target, relay_outbox}` re-exports) — so they **survive**, relocated from the
  deleted `workflow/` dir to their honest home `outbox/` (they are outbox delivery primitives),
  re-exported unchanged from `lib.rs` so no caller path breaks. We add a generic **relay
  reactor** (twin of the approval reactor) so a staged effect is actually delivered — the
  driver github-workflow used to provide.
- **Webhook source as node:** `flows/source.rs` already arms *extension* source nodes onto a
  host-allocated series. The webhook source is the built-in case: its series is the core
  webhook's own `webhook:{ws}:{id}` (not a fresh `flow:…` series), so arming = subscribe +
  fire-a-run-per-sample; no ext `arm`/`disarm` tool. Config is just `{webhook_id}`.

**Alternative rejected:** keep rules + workflow as separate engines and merely cross-call
them. Rejected — it leaves three schedulers, three run models, and three places to add a
guard; the whole point is one spine. The workflow's generic primitives are *strictly weaker*
than the flow engine's (no DAG, no resume, no per-node config), so folding them in loses
nothing and deletes a large surface.

## How it fits the core

- **Tenancy / isolation:** every new record + series is `ws`-scoped. The flow-approval
  reactor and the relay reactor select the workspace namespace, so a ws-B pass can only touch
  ws-B resolutions/effects (mandatory isolation test). The webhook source subscribes to
  `webhook:{ws}:{id}` — a ws-B flow cannot arm a ws-A hook.
- **Capabilities:** `rules.eval` is gated `mcp:rules.eval:call` at the bridge, then the cage's
  per-source `caps::check` (unchanged). The `rhai`/`rule` nodes dispatch it under the caller's
  own authority (`caller ∩ grant`, no widening) — a flow whose node calls `rules.eval` without
  the cap is **denied at that node**. The approval-gate and outbox-sink nodes dispatch the
  generic `inbox.*`/outbox verbs under the caller — same deny path. Deny-test per new verb.
- **Placement:** either. The reactors are symmetric (config picks which node ticks them),
  never an `if cloud`.
- **MCP surface:** one new host verb, `rules.eval` (a `run`-shaped read/eval; **no** CRUD — a
  rule's CRUD is `rules.save/get/list/delete`, unchanged). The nodes themselves are not new
  MCP verbs — they are descriptors dispatched through the existing `flows.run` path. The
  webhook source adds **no** verb (it reads `webhook.list` for the picker; arming is internal).
- **Data (SurrealDB):** the approval-gate node writes a `needs:approval` inbox item + a
  small parked-run marker (reusing `flow_node_buffer`); the outbox-sink stages an `lb_outbox`
  effect. The webhook source writes its armed marker in `flow_node_state` like any source. No
  new tables.
- **Bus (Zenoh):** the webhook source subscribes to the ingest series `webhook:{ws}:{id}`
  (motion → one run per sample). Everything durable stays in the store (state vs motion).
- **Sync / authority:** node-local run engine; the reactors are durable scans (source of
  truth is the store), so a restart re-reads and never misses a resolution/effect.
- **Secrets:** none new. The webhook credential lives in the core webhook service (`lb-secrets`);
  the source node never sees it.
- **One responsibility per file:** `rules/eval.rs` (the verb), `execute_node/rule.rs` (the
  saved-rule node leg), `execute_node/approval.rs` (the gate leg), a `flow_approval_reactor/`
  and `relay_reactor/` folder each (id/react/spawn), `flows/source.rs` gains the webhook arm
  (kept under 400). The deleted files remove ~1500 lines net.

## Example flow

A "webhook → evaluate → gate → deliver" flow, all generic nodes:

1. **`webhook` source** (config `{webhook_id: "gh-hook"}`) is armed when the flow enables:
   the host subscribes to `webhook:acme:gh-hook`. A real POST to `/hooks/acme/gh-hook`
   (verified by the core service) writes one ingest sample; the source fires a run, the
   sample's payload as the envelope.
2. **`rhai`** node (`rules.eval`) runs an inline rule over `payload`, emits
   `{payload, findings}`; the findings render on the canvas.
3. **`approval` gate** writes `needs:approval` for team `reviewers` and **parks** the run.
4. A reviewer resolves the item `Approved`; the **flow-approval reactor** calls `flows.resume`.
5. On resume the gate settles through; the **`sink` (target=outbox)** node stages a
   must-deliver effect; the **relay reactor** delivers it through the registered `Target`
   with retry/backoff, dead-lettering a poison effect.

No provider name appears anywhere in the core path — the delivering `Target` is opaque.

## Testing plan

Mandatory categories (per `scope/testing/testing-scope.md`), all on real infra (`mem://`
store, real bus, real caps, real gateway — rule 9), seeded via the real write path:

- **Capability-deny (per verb/node):**
  - `rules.eval` without `mcp:rules.eval:call` → denied (opaque).
  - a flow whose `rhai`/`rule` node calls `rules.eval` under a caller lacking the cap →
    the node settles `err`/denied (no widening).
  - the approval-gate node under a caller lacking `mcp:inbox.*` → denied at the node.
- **Workspace-isolation:**
  - a ws-B flow-approval reactor pass leaves a ws-A parked run parked (never resumes it).
  - a ws-B relay pass never delivers a ws-A effect.
  - a ws-B webhook source cannot arm/fire on a ws-A hook's series.
- **The convergence, end to end (real store):**
  - `rules.eval` on an inline body returns `{output, findings, log}`; findings flow onto the
    node envelope.
  - a saved-rule `rule` node runs the stored rule by name + params.
  - per-flow concurrency: with a live run, `skip` no-ops, `queue` enqueues, `restart`
    cancels-and-restarts — proven in the cron reactor AND `flows.run`.
  - per-node `timeout_ms`: a node exceeding it settles `err:"timeout"`, downstream gated.
  - approval gate: run parks → resolve `Approved` → reactor resumes → completes; a `Rejected`
    resolution cancels the run.
  - outbox sink + relay reactor: effect staged → relay delivers → marked delivered;
    fail-then-succeed retries; poison dead-letters.
- **Webhook source arm/fire (webhooks-scope testing plan):** seed a real webhook, enable a
  flow with a `webhook` source, write a real ingest sample on `webhook:{ws}:{id}` (the real
  path the gateway route uses), assert exactly one run fires with the sample payload.
- **Frontend (Vitest):** the flows node-picker lists the new `rule`, `approval`, and
  `webhook`-source descriptors; the webhook source's `{webhook_id}` picker lists hooks from a
  real seeded `webhook.list` (`pnpm test`); the real inbound-hit path (`pnpm test:gateway`).
- **Deletion safety:** after slice 4, `cargo build --workspace` is green with **zero**
  dangling references to `workflow`/`PrSpec`/`github-workflow`/`github-target`.

## Risks & hard problems

- **`relay_outbox` losing its only driver.** Deleting github-workflow removes the loop that
  drove `relay_outbox`. Without a replacement, a staged outbox effect never leaves. The
  **relay reactor** (new) is load-bearing, not optional — it is what makes the outbox-sink
  node actually deliver. Tested directly.
- **Park/resume correctness for the approval gate.** The gate must be idempotent across
  re-drive: writing the inbox item once, parking, and on resume distinguishing
  "still pending" (park again) from "Approved" (settle) from "Rejected" (fail/cancel). Reuse
  the `delay` node's exact park shape + the generic approval reactor's guarded-transition
  discipline.
- **Deletion blast radius.** The workflow touches the gateway routes, credentials, node
  wiring, three role crates, and ~8 host test files. A missed reference is a red build. The
  slice ends with a workspace-wide grep gate.

## Open questions

1. **Concurrency-policy default.** — *Resolved in build:* the policy is a per-flow field
   (`Concurrency::{Queue, Skip, Restart}`, additive serde default **`queue`**) honored uniformly
   at **every** fire seam — the cron/interval/inject reactors AND the manual `flows.run` — so
   behaviour is one rule, not a per-caller special case. The default is `queue` because it
   **preserves the established behavior** (two manual runs of a flow both run; a cron tick never
   suppresses a still-running prior tick), so a flow written before this field is unchanged. An
   author opts into `skip` (one live run at a time) or `restart` (latest-wins control loop). A
   `subflow` child run is NOT guarded (it is a child of its parent, not an independent firing).
2. **Where the outbound `Target` is registered.** A host-registered map keyed by effect
   `target` string (opaque), with the test `Target` seeded in tests and a real one supplied by
   an extension later. Confirm the registration seam stays provider-free (it does — the key is
   data).
3. **`github-webhook` role + `github-bridge` ext.** `github-webhook` calls the deleted
   `ingest_via_bridge` and is GitHub-inbound — **delete it too** (its only role is the deleted
   workflow; the new generic webhook backend replaces it). `github-bridge` is a wasm ext
   excluded from the workspace build; remove every *core* reference (the cap string in the
   deleted `node/src/github.rs`, the tests) but leave the sample ext directory. — *Resolved.*

## Related

- README `§6.2` (state vs motion), `§6.10` (outbox/durability), `§3` (the non-negotiables).
- `scope/flows/flows-scope.md`, `flow-run-scope.md`, `flow-message-envelope-scope.md`,
  `triggers-lifecycle-scope.md`, `flow-runtime-control-scope.md`, `data-nodes-scope.md`,
  `chains-retirement-scope.md` (the pattern for retiring a converged engine).
- `scope/rules/rules-engine-scope.md`, `rules-approvals-scope.md` (the generic approval
  reactor this reuses).
- `scope/ingest/webhooks-scope.md`, `scope/auth-caps/api-keys-scope.md` (the webhook backend
  the source node wraps).
- `scope/coding-workflow/*` (marked retired by this scope).
- `public/flows/flows.md` (promotion target).
</content>
</invoke>
