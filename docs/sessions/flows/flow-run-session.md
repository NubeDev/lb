# Flows — the durable run engine (slice 2)

- Area: flows
- Status: shipped (green)
- Scope: [`scope/flows/flow-run-scope.md`](../../scope/flows/flow-run-scope.md) (owns Decisions **1, 6,
  7, 8, 9, 11, 12** run-side).
- Spine: [`scope/flows/flows-scope.md`](../../scope/flows/flows-scope.md).
- Session: this file. Prev: [slice 1 — node descriptor](node-descriptor-session.md). Next: extension-nodes (3) → triggers (4).

## What this slice is

The **durable run engine** over `lb-jobs` (flow-run-scope) — how a node-graph turns into work. A run
is a `flow_run` **coordinator record + one `lb-jobs` job per node** (a `flow-step`), driven by the
`chains` frontier driver **ported verbatim** (Decision 8): validate-at-save, in-degree-0 frontier,
CAS step-claim (`Enqueued→Running`), `on_step_done` release, `FailurePolicy = Halt | Continue`. The
run pins `flow.version` (Decision 1); editing writes a new version; a structural edit during a
suspend is rejected as a live-run patch and `ResumePointDrift`s a resume against a moved version.
Plus the full `flows.*` run MCP surface + `flows.runs.list` (reattach) + the canonical `coalesce` enum.

**This is not a new engine.** The frontier driver, CAS claim, run-store shape, and binding grammar
are the chain engine's, generalised to the typed node payload (Decision 6).

## What shipped

### Pure crate additions (`lb-flows`)
- `coalesce.rs` — the canonical `Coalesce { strategy: latest|leading|trailing|sample, window_ms }`
  enum (flow-run-scope "Fan-out posture"). The one vocabulary triggers + dashboard-binding reference.

### Host `flows` service (`crates/host/src/flows/`) — one responsibility per file
- `record.rs` — `ClaimState` / `FlowRunRecord` (coordinator + pinned `flow_version`) /
  `FlowStepRecord` (per-node CAS claim + outcome + `patched_config` override). Tables re-exported
  from `lb-flows::table`.
- `run_store.rs` — the durable backend ported from chains: `create_run` (pins version), `claim_step`
  (CAS exactly-once), `record_outcome` (upserts `flow_node_state` last-value on Ok — Decision 5),
  `ready_dependents` / `skip_subtree` (Halt) / `finalize_if_complete`, `resolve_node_bindings`,
  `merged_params_with_inputs` (Decision 9 read-side — every run reads retained `flow_input` values).
- `coordinator.rs` — `start` + `drive` (the frontier loop, ported from the chain coordinator).
- `execute_node.rs` — run ONE node: CAS-claim → resolve bindings → dispatch by type → record outcome
  → release/prune. Dispatch: `tool` (generic MCP verb under `caller ∩ grant`), `rhai` (`rules.run`,
  output unwrapped like chains), `sink` (`series`→`ingest.write` / `outbox`→the outbox must-deliver /
  `channel`·`inbox`→`inbox.record`), `subflow` (parks on a pinned child run), `trigger` (firing
  payload), ext node (its bound tool). Every leg goes through `call_tool` (the one chokepoint) so
  each node-tool's own gate re-checks — **no widening**.
- `save.rs` — `flows.save` (DAG-validated + every node config re-validated against its descriptor's
  schema, version bumped on edit — Decision 1 + config_version evolution) / `get` / `list` / `delete`.
- `run.rs` — `flows.run {id,params}→{run_id}` (validate, pin version, create job, drive, complete) +
  `flows.resume` (re-drive + drift guard) + `run_flow_to_completion` (shared by `flows.run` AND a
  subflow node — the child IS a real pinned run).
- `lifecycle.rs` — `flows.suspend` / `flows.cancel` (terminal + non-resumable).
- `patch_run.rs` — `flows.patch_run` (config-only to an UNEXECUTED node, validated against the pinned
  descriptor — Decision 1/12; the override rides a dedicated `patched_config` field the executor reads).
- `runs.rs` — `flows.runs.get` (per-node snapshot + pinned version, the `ResumePointDrift` surface)
  + `flows.runs.list` (reattach: find the active `run_id` from a `flow_id`).
- `error.rs` — `FlowsError` (incl. `ResumePointDrift`). `mod.rs` — the dispatch (all verbs).

### Wiring
- `tool_call.rs` — the `flows.` `is_host_native` + dispatch arm (from slice 1; now exercised by all
  run verbs). The run engine re-enters `call_tool` for node execution (boxed at the recursion points
  — a flow may run a flow / call a flow verb).

## How it fits the core (the platform checklist)
- **One datastore / no new persistence** — `flow_run` / `flow_step_output` mirror the chain run-store
  (Decision 6); `flow_node_state` last-value + `flow_input` retained values. ✔
- **Symmetric nodes** — the engine is placement-free; a run executes on the node that owns its
  workspace authority, like any job. No `if cloud`. ✔
- **Capability-first / composition, never widening** — `flows.run` plus every node-tool's own gate
  under `caller ∩ grant`; a tool node calling a verb the caller lacks is denied at that node (the
  headline no-widening test), recorded `Err`, run continues per `FailurePolicy`. ✔
- **Workspace is the hard wall** — every record is `…:{ws}:…`; ws-B cannot run/get/list/patch a ws-A
  run. ✔
- **State vs motion** — `flow_node_state` last-value is state; the node's series carries live ticks
  (slice 3/4). Must-deliver sinks stage outbox effects. ✔
- **Exactly-once, two layers (Decision 8)** — the CAS claim (`Enqueued→Running`) owns cross-node
  exactly-once under redelivery; a re-drive/resume no-ops already-run nodes. ✔

## Testing (real infra, no mocks)
`host flows_run_test` **12** (real `mem://` store + real `lb-jobs` + real caps + flows/rules seeded
via the real write path):
- save-rejects-a-cyclic-DAG · linear-rhai-flow-runs-to-success (version pin + per-node ok) ·
  diamond-frontier-runs-in-dependency-order · halt-policy-skips-failed-subtree ·
  **no-widening** (a rhai node denied without `mcp:rules.run` → node `err`, run `failed`) ·
  **capability-deny** (`flows.run` without the cap) · **workspace-isolation** (ws-B can't get/run a
  ws-A flow) · **structural-edit→new-version** (re-save bumps v1→v2) · **patch_run** rejects an
  executed node · **resume-is-idempotent** (a re-drive is a no-op — exactly-once) ·
  **runs.list reattach** · **subflow-parks-on-child** (parent step folds the child's terminal outputs).

```
cargo test -p lb-flows                 → 26 green (incl. 2 new coalesce)
cargo test -p lb-host --test flows_run_test   → 12 green
cargo test -p lb-host --test flows_nodes_test → 5 green
cargo build --workspace                → green
cargo fmt --check                      → clean
```

## Decisions made this slice (consistent with the spine)
- **Subflow "park" realised as an inline child drive (v1).** A `subflow` node creates a REAL pinned
  child `flow_run` and the parent step waits for the child to reach terminal (then maps child outputs
  → parent), driven inline by the same coordinator. This is functionally the Decision-11 "park on a
  child run" (the child IS a pinned run; the CAS claim keeps it exactly-once; parent/child pin
  versions independently). A reactor-driven park (suspend-parent-on-child-start, resume-parent-on-
  child-terminal) is a noted follow-up that does not change the run's outcome. Traces to Decision 11.
- **`patch_run` override rides a dedicated `patched_config` field** on the step record (read by the
  executor in place of the flow's node config), never overloading the recorded output — keeps the
  Decision-1/12 "config-only to an unexecuted node" clean and the recorded output honest.
- **Resume drift guard compares the run's pinned `flow_version` to the flow's current version.** A
  structural edit during suspend writes a new version (the live run finishes on its pinned one); a
  forced resume against a moved version fails cleanly as `ResumePointDrift`. (A pinned-graph cache
  landing with the full version-pin follow-up makes this exact for an in-place edit of the pinned
  version too.) Traces to Decision 1/12.
- **Rhai node output is unwrapped** to the scalar/grid JSON a downstream `${steps.x.output}` reads
  (the chain `output_json` convention), so flow bindings and chain bindings see the same shape.

## Open questions / next
- Slice 3 (`extension-nodes`) makes the ext-node dispatch resolve the descriptor's exact `<ext>.<tool>`
  binding + the gated `caller ∩ install-grant` callback (both transports), and ships the worked `mqtt`
  reference extension (source arm/disarm → `ingest.write` → series, must-deliver sink).
- Slice 4 (`triggers-lifecycle`) adds the five trigger kinds, `flows.enable`, the `react_to_flows_cron`
  clock-scan + `reconcile_flows` owner-election/arming loop, placement, and guarded teardown.
- `flows.watch` SSE (the preferred canvas live source) is a named follow-up; the canvas falls back to
  a bounded `flows.runs.get` poll until it lands.
