# Flows — the Node-RED-style message envelope (`payload`/`topic`, auto-wire on connect)

- Area: flows
- Status: shipped (green) — engine + binding grammar + descriptors + tests + frontend + docs
- Scope: [`scope/flows/flow-message-envelope-scope.md`](../../scope/flows/flow-message-envelope-scope.md)
- Builds on: [`flow-multi-trigger-reactive-session.md`](./flow-multi-trigger-reactive-session.md)
  (per-wire subgraph runs + the counter trap this slice removes) and
  [`flow-run-session.md`](./flow-run-session.md) (`resolve_node_bindings` that gains auto-wire).
- Spine: [`flows-scope.md`](../../scope/flows/flows-scope.md) Decisions 4 (binding grammar redefined),
  5 (`flow_node_state` now holds an envelope), 9 (inject/retained = `payload`).

## The ask

> Make flows feel like Node-RED: a message is `{ payload, topic, ... }`; connecting A→B flows A's
> output to B with **no binding typed**; `topic` carries down a chain. **Clean breaking change** — flows
> is in dev, no migration. Kill the implicit-throughput counter trap while we're here.

The scope's **Decisions D1–D11** are the contract; there were no open questions. Followed verbatim.

## What shipped (the slice, end to end)

1. **Binding grammar** ([`rust/crates/flows/src/binding.rs`](../../../rust/crates/flows/src/binding.rs)) —
   `NodeOutput` now holds the **whole envelope**. `${steps.x}` → the whole envelope; `${steps.x.<dot.path>}`
   → a JSON-pointer-style field walk (`payload`, `topic`, `findings`, `payload.items.1`, …; missing →
   `null`); `${params.y}` unchanged; literal otherwise. The special-cased `.output`/`.findings` forms are
   **gone** (D5) — they are now ordinary field paths. "Whole-reference only, no interpolation" kept.
2. **Auto-wire + carry-forward** ([`host/src/flows/run_store.rs`](../../../rust/crates/host/src/flows/run_store.rs)
   `resolve_node_bindings`) — single upstream + no `with.payload` → `inputs` = the upstream's full
   recorded envelope (copy, D3). It now returns `ResolvedInputs { inputs, carry }`; `carry` = inputs
   minus `payload`. `record_outcome` records `{ ...carry, ...emitted }` so `topic` propagates (D4). A
   join (≥2 upstreams) carries nothing.
3. **Node dispatch** ([`host/src/flows/execute_node.rs`](../../../rust/crates/host/src/flows/execute_node.rs)) —
   every builtin reads `inputs["payload"]` and emits the D6 envelope. `NodeOutcome::Ok` is now
   `{ emitted, carry }` (the executor injects `carry` after dispatch). `counter` gained an explicit
   `mode: "tick" | "throughput"` (default `tick` = +`step` every firing regardless of payload). `rhai`:
   an object return carrying a `payload` key IS the envelope (`return msg`), else it is the new payload;
   rules `findings` ride the `findings` field. `sink` destination = `msg.topic ?? config.name`, writes
   `msg.payload`, emits a pass-through `{ payload }`. `tool` merges `config.args` with `payload` when
   `payload` is an object; the verb result becomes the emitted `payload`. `subflow` reads `payload` in,
   emits the child's folded outputs as `payload`.
4. **Descriptors** ([`flows/src/builtins.rs`](../../../rust/crates/flows/src/builtins.rs)) — every built-in
   port renamed to `payload`/`topic`/`findings` (grepped out every `items`/`value`/`output` literal). A
   new test asserts no built-in carries a non-envelope port. The **join lint** lives in `validate_flow`
   ([`flows/src/model.rs`](../../../rust/crates/flows/src/model.rs)): a node with ≥2 `needs` and no
   `with.payload` is `DagError::UnboundJoin` → `FlowsError::BadInput` at save.
5. **inject / retained inputs** — unchanged signature; the retained `value` is framed as the node's
   `payload`. A run reading a retained input node gets `inputs = { payload: <value> }` (D8).
6. **node_state / snapshot** — `flow_node_state` stores the whole envelope; `flows.node_state` /
   `flows.runs.get` return it unchanged (D9).
7. **Frontend** ([`ui/src/features/flows/flowGraph.ts`](../../../ui/src/features/flows/flowGraph.ts)) —
   `snapshotValues`/`nodeStateValues` map the envelope to its `payload` for the value badge via a new
   `payloadOf()` (falls back to the whole envelope when there is no `payload` key, D10).

## Decisions made where the scope left a case unspecified

- **Auto-wire over a non-object upstream** (a `Continue`-null): wrap it as `{ payload: <value> }` so the
  downstream node still reads a well-formed message rather than an empty `inputs`. Documented in
  `resolve_node_bindings`.
- **`NodeOutcome::Ok` shape**: rather than thread `carry` through every dispatch arm, dispatch returns
  `emitted` only and `execute_one` attaches `carry`. Each arm calls `NodeOutcome::ok(emitted)`; the
  carry merge is one place. Keeps the per-builtin arms about the envelope, not the propagation rule.
- **The vestigial `findings` step column**: left on `FlowStepRecord` (additive serde default, harmless)
  but no longer read for binding — findings now ride the envelope's `findings` field.

## Test sweep (the bulk of the work)

Updated every test that encoded the old shape and added the scope's new cases. Mandatory categories
kept: capability-deny (`no_widening_tool_node_denied…`, `capability_deny_run…`) and workspace-isolation
(`workspace_isolation_ws_b_cannot_see_ws_a_flow`) — assertion shapes moved to `payload`.

New cases (`flows_run_test.rs`): `auto_wire_flows_the_envelope_end_to_end_with_no_with` (3-node linear
chain, NO `with`), `save_rejects_a_join_with_no_payload_binding` (the lint → `BadInput`, then passes once
bound), `topic_carries_forward_down_the_chain`, `rhai_return_msg_round_trips_the_envelope`,
`counter_tick_mode_does_not_jump_by_payload_size` (the fail-before for the trap),
`counter_throughput_mode_adds_payload_size`. Binding field-paths in `binding.rs` unit tests; the join
lint in `model.rs`; the no-stray-port lint in `builtins.rs`. Sink: added
`sink_destination_uses_msg_topic_over_config_name`. Touched `flows_runtime_control_test`,
`flows_triggers_test`, `flows_multi_trigger_test` for the envelope shape.

## Green output

```
cargo build -p lb-host           → Finished (green)
cargo fmt                        → clean

cargo test -p lb-flows           → 33 passed; 0 failed
cargo test -p lb-host (flows_*):
  flows_ext_test            5 passed; 0 failed
  flows_multi_trigger_test  5 passed; 0 failed
  flows_nodes_test          5 passed; 0 failed
  flows_run_test           19 passed; 0 failed   (incl. the 6 new envelope cases)
  flows_runtime_control_test 12 passed; 0 failed
  flows_sink_test           4 passed; 0 failed   (incl. msg.topic routing)
  flows_triggers_test       9 passed; 0 failed

cd ui && pnpm test (flowGraph)   → 29 files / 207 tests passed
```

The separate `chains` engine (its own `run_store::resolve_bindings`, untouched here) was re-run in
isolation to confirm no spillover: `chains_test` 6 passed (it is bus-timing-slow under the full parallel
suite but green on its own).

## No debug entry

Nothing broke non-trivially: the failing tests were the *expected* sweep failures (old shape), not bugs
in the new path. The implicit-throughput trap that motivated D7 is recorded under the prior slice's
[`flow-level-cron-rejects-multiple-triggers.md`](../../debugging/flows/flow-level-cron-rejects-multiple-triggers.md);
this slice removes its root cause by making `counter` mode explicit, guarded by the new regression test.

## Follow-up (explicitly out of scope here)

[`flow-dashboard-binding-ux-scope.md`](../../scope/flows/flow-dashboard-binding-ux-scope.md) — the picker
offering `payload`/`topic` ports and read views defaulting to `payload` — depends on this slice and is a
separate session.
