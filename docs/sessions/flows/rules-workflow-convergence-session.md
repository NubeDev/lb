# Flows — rules + workflow convergence (session)

- Date: 2026-07-05
- Scope: ../../scope/flows/rules-workflow-convergence-scope.md
- Stage: S8+ (data plane shipped) — flows is the spine; this converges the last two side-engines onto it.
- Status: done

## Goal

Converge the rules engine and the GitHub "workflow" module INTO flows as generic nodes, add the flow
webhook **source** node, and HARD-DELETE every GitHub/coding-specific piece — leaving one engine to
reason about and **zero** provider names in any core crate (rule 10). Exit gate: `cargo build/test
--workspace` green (minus the known pre-existing reds), the mandatory cap-deny + ws-isolation tests
green, and a workspace-wide grep proving no dangling `workflow`/`PrSpec`/`github-*` callers.

## What changed

**Slice 1 — rules ARE flow nodes.**
- New `rules.eval` host verb ([`rules/eval.rs`](../../../rust/crates/host/src/rules/eval.rs)): the
  flow-envelope rule entry — `{body|rule_id, envelope, params, timeout_ms}` in → `{output, findings,
  log}` out. Same `RuleEngine`, same cage, same per-source `caps::check` as `rules.run`; only the arg
  mapping (envelope fields → rule params, each a top-level rhai var) + the optional `timeout_ms` differ.
  Gated `mcp:rules.eval:call`; added to the member grant in `session/credentials.rs`.
- Rewired the existing `rhai` node ([`execute_node/core.rs`](../../../rust/crates/host/src/flows/execute_node/core.rs))
  from dispatching `rules.run` to `rules.eval` (its descriptor already declared `tool: "rules.eval"`
  but nothing served it). Added a sibling `rule` node that runs a **saved** rule by id + params.
- `rules_run` gained an `Option<RuleLimits>` param so `rules.eval` can override the cage deadline;
  the 5 direct test callers pass `None`.

**Slice 2 — engine guards.**
- Per-flow `Concurrency::{Queue, Skip, Restart}` on the `Flow` model
  ([`crates/flows/src/model.rs`](../../../rust/crates/flows/src/model.rs), additive serde default
  **`queue`** — preserves the established behavior so a pre-existing flow is unchanged),
  enforced at every fire seam by [`flows/concurrency.rs`](../../../rust/crates/host/src/flows/concurrency.rs):
  `flows_run` (cron/interval/inject) AND `flows_run_async` (manual). `skip` drops an overlapping firing,
  `restart` cancels the live run(s), `queue` overlaps.
- Per-node `timeout_ms`: `execute_one` wraps dispatch in a `tokio::time::timeout`; a node that exceeds
  it settles `err:"timeout"` (subtree gated like any failure). Exposed on `tool`/`rhai`/`rule`/`sink`/
  `subflow` descriptors. The `rhai`/`rule` cage deadline is also driven by the same `timeout_ms`.

**Slice 3 — cron:** verified unchanged; the concurrency guard now also applies to cron fires.

**Slice 4 — HARD DELETE workflow, keep the generic pieces.**
- **Relocated** (github-free, kept): the `Target` trait + `relay_outbox` loop → `outbox/{target,relay}.rs`
  (they are outbox delivery primitives, still used by the reminders/approval-release tests + the ROS
  ext through `lb_host::{Target, relay_outbox}`); the workspace directory → `directory/` (the generic
  reactor-directory). Re-exported unchanged from `lib.rs` so no caller path broke.
- **New generic nodes/reactors:** the `approval` gate node
  ([`execute_node/approval.rs`](../../../rust/crates/host/src/flows/execute_node/approval.rs)) parks the
  run on the existing `Dispatched::Park` machinery + writes a `needs:approval` inbox item; the
  flow-approval reactor ([`flows/react_approval.rs`](../../../rust/crates/host/src/flows/react_approval.rs))
  resumes on `Approved` / cancels on `Rejected`. The outbox relay reactor
  ([`outbox/relay_reactor.rs`](../../../rust/crates/host/src/outbox/relay_reactor.rs)) drives
  `relay_outbox` on a tick with a supplied `Target` — the generic replacement for the deleted
  github-workflow driver that was the ONLY thing driving the relay. The outbox-sink is the existing
  `sink(target=outbox)` leg.
- **Deleted** (no migration): `crates/host/src/workflow/*` (pr_spec, ingest, ingest_via_bridge, triage,
  start_job, request_approval, resolve_approval, react, effect, tool, error, authorize, mod); the roles
  `role/github-workflow`, `role/github-target`, **and `role/github-webhook`** (it called the deleted
  `ingest_via_bridge` — its whole job was the GitHub inbound ingress the new generic webhook backend
  replaces); `node/src/github.rs` + its `main.rs` mount; the gateway `routes/workflow.rs` + route
  wiring + the `mcp:workflow.*` member caps; the host `workflow_*`/`github_bridge_*` tests; the mixed
  gateway test split into `assets_routes_test.rs`. Renamed the misleadingly-named local
  `tool_call::call_workflow_tool` (dispatches generic inbox/outbox) → `call_inbox_outbox_tool`.

**Slice 5 — the webhook SOURCE node.**
- New built-in `webhook` source descriptor (config `{webhook_id}`) + a series-event reactor
  ([`flows/react_source.rs`](../../../rust/crates/host/src/flows/react_source.rs)): per enabled flow's
  `webhook` source it reads the core webhook's series `webhook:{ws}:{id}` for samples past a durable
  `last_seq` cursor (new field on `FlowTriggerState`), fires one run per new hit (entry = the source
  node, the hit payload as the envelope), advances the cursor. Fire-once, restart-safe, ws-walled. The
  node owns no endpoint/credential — it wraps the already-shipped webhook backend (out of scope).
- All three new reactors are wired into the flow reactor tick (`reactor_loop.rs`) with the added caps
  (`flows.resume`/`flows.cancel`/`series.read`).
- UI: registered the `webhook`/`scroll`/`shield-check` icons in `flowIcons.ts` (the palette is
  data-driven, so the nodes themselves need no UI change).

## Decisions & alternatives

- **`Target`/`relay_outbox` relocated to `outbox/`, not deleted.** They are github-free and still
  depended on by the reminders + approval-release tests and the ROS ext. Their honest home is the
  outbox module (delivery primitives). Rejected: deleting them (would break cluster-B) or leaving them
  in a vestigial `workflow/` (a dead module named after a deleted concept).
- **Approval gate uses the generic `inbox` verbs, not the workflow's `request_approval`.** The latter
  was bolted to `PrSpec`. The gate writes a plain `needs:approval` item keyed `flow-approval:{run}:{node}`
  and parks; a tiny reactor resumes it. Zero provider coupling.
- **A series-event reactor, not a live subscription.** The "sample lands → fire a run" path did not
  exist (the STATUS's "event trigger covers it" was aspirational). A durable cursor scan (the cron
  reactor's exact shape) is restart-safe and never drops a hit — a dropped-on-restart live subscription
  would not be. Rejected: a long-lived Zenoh subscription per source (loses hits across a restart).
- **`github-webhook` deleted too** (beyond the scope's explicit two roles). It compiles only against
  the deleted `ingest_via_bridge` and is pure GitHub-inbound; keeping it would be a red build. Its
  replacement is the generic webhook backend + this source node. Confirmed its only callers were the
  manifest + `node/src/github.rs` (both deleted).
- **`github-bridge` wasm ext left in place.** It is `exclude`d from the workspace build (not compiled
  by `cargo build --workspace`); its only *core* reference was the cap string in the deleted
  `node/src/github.rs`. Left as a standalone sample; every core reference is gone (rule 10 holds).
- **Concurrency default `queue`, uniform across all fire seams.** `queue` preserves today's behavior
  (overlapping manual runs both run; a cron tick never suppresses a still-running prior tick), so the
  new field is invisible to every existing flow; `skip`/`restart` are opt-in. Discovered during the
  full-suite run: a `skip` default broke the established "two manual runs both succeed" +
  cron/flipflop-overlap tests — `queue` is the behavior-preserving default (see scope Open Q1). A
  `subflow` child run is exempt (it is a child, not an independent firing).

## Tests

Real store (`mem://`), real caps, real `lb-jobs`, real ingest buffer, real reactors — no mocks (rule
9); the only mocked thing is the delivery `Target` (a true external, testing §3). New file
[`crates/host/tests/rules_workflow_convergence_test.rs`](../../../rust/crates/host/tests/rules_workflow_convergence_test.rs)
— **14/14 green**, covering the mandatory **capability-deny** (`rules_eval_denied_without_the_cap`) and
**workspace-isolation** (`ws_b_source_reactor_never_fires_ws_a_hits`,
`ws_b_approval_reactor_never_resumes_ws_a_run`) categories plus every slice's functional path:

```
running 14 tests
test rules_eval_denied_without_the_cap ... ok
test outbox_sink_effect_is_delivered_by_the_relay ... ok
test concurrency_restart_cancels_the_live_run ... ok
test rhai_node_runs_rules_eval_end_to_end ... ok
test per_node_timeout_settles_err_timeout ... ok
test approval_gate_parks_then_resumes_on_approved ... ok
test concurrency_skip_drops_the_overlapping_firing ... ok
test rules_eval_maps_envelope_to_params_and_returns_findings ... ok
test saved_rule_node_runs_a_stored_rule_by_id ... ok
test ws_b_approval_reactor_never_resumes_ws_a_run ... ok
test approval_gate_cancels_the_run_on_rejected ... ok
test ws_b_source_reactor_never_fires_ws_a_hits ... ok
test concurrency_queue_lets_runs_overlap ... ok
test webhook_source_fires_a_run_per_hit ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.71s
```

Frontend: `cargo build --workspace` + `cargo fmt --check` green; UI `pnpm test` **631/631**;
`pnpm test:gateway` on `FlowsCanvas.gateway.test.ts` **13/13** (extended to assert the picker lists
`webhook`/`rule`/`approval` from the **real** registry and that the webhook source's config is
`{webhook_id}`).

**The rhai rewire + new spine nodes rippled into existing flows tests — all fixed:**
- The `rhai` node now dispatches `rules.eval`, so every flows test that RUNS a rhai node needs
  `mcp:rules.eval:call` (added beside `mcp:rules.run:call` in flows_run/flipflop/multi_trigger/triggers
  tests); the `no_widening` test now revokes `mcp:rules.eval:call` (the rhai node's real dispatch).
- The three new spine node types shifted the built-in registry, so `flows_nodes_test`'s `BUILTINS` list
  (+`data_pack_nodes_are_envelope_transforms`'s slice index) were updated to 32.
- The concurrency default surfaced as `queue` (see Decisions): a `skip` default broke the established
  "two manual runs both succeed" + cron/flipflop-overlap tests; `queue` preserves today's behavior.

After the fixes the affected suites are green: flows_run 7/7, flipflop 5/5, multi_trigger 5/5,
triggers 4/4, nodes 25/25, plc 17/17, convergence 14/14.

**Pre-existing reds — NOT this session's regressions (verified against base master `4c733cd`):**
`panel_test` (4 fail: a dashboard "unknown view 'STALE'" hydration issue — fails identically on base),
`agent_routed_test` (fails on base — the task's known exclusion), and `proof_panel_test` (10: its
`proof-panel` wasm ext isn't built in this worktree — an env prereq, not a code fault). None touch the
rules/flows/outbox/directory code this session changed.

Deletion-safety grep: zero dangling `workflow`/`PrSpec`/`github-workflow`/`github-target` callers (the
only remaining hits were two stale doc comments, now corrected, + the excluded `github-bridge` sample).

## Debugging

None persisted — the test failures during authoring were my own test bugs (rule bodies used
`params.payload` instead of the cage's top-level `payload` var; `inbox.resolve` takes `item_id` not
`{channel,id}`; a `store:rule:read` cap was missing; a test helper read `output` off the step wrapper),
all fixed in-session before any behavior shipped. No product-code defect → no `debugging/` entry.

## Public / scope updates

- Promoted to [`public/flows/flows.md`](../../public/flows/flows.md) (convergence section) +
  `public/SCOPE.md`.
- Scope open questions all resolved in the scope doc (concurrency default; `Target` registration seam;
  `github-webhook`/`github-bridge` disposition).
- Marked `docs/scope/coding-workflow/*` **retired** (mirrors `chains-retirement-scope.md`).

## Skill docs

n/a for a NEW skill: the drivable surface added is one MCP verb (`rules.eval`) that behaves like the
already-documented `rules.run`, plus flow node descriptors reached through the existing `flows.*`
surface (covered by `skills/rules/` + the flows skill). No new agent-/API-drivable task shape.

## Dead ends / surprises

- The `rhai` node already *declared* `tool: "rules.eval"` but dispatched `rules.run` and no `rules.eval`
  verb existed — the convergence's slice 1 was finishing a half-wired seam, not adding a new one.
- The "sample → fire a run" event path did not exist at all; slice 5 had to build the series-event
  reactor, not just a descriptor.
- `Target`/`relay_outbox`/`directory` live inside `workflow/` but are github-free and cross-depended;
  the deletion was really a *relocation* of three files + a hard-delete of the rest.

## Follow-ups

- A real delivery `Target` for the outbox-sink is an extension (supplied to `spawn_relay_reactors` by
  the binary) — none ships in core by design (rule 10). The node + relay machinery are proven; wiring a
  concrete adapter is a separate ext slice.
- `STATUS.md` updated (this slice + the workflow retirement).
</content>
