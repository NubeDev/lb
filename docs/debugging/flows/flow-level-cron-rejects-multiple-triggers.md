# flows: a flow could hold only ONE trigger — `flows.save` rejected a second cron, and one run fired the whole graph

- Area: flows
- Status: resolved
- Date: 2026-06-30
- Scope: [`scope/flows/flow-multi-trigger-reactive-scope.md`](../../scope/flows/flow-multi-trigger-reactive-scope.md)
- Session: [`sessions/flows/flow-multi-trigger-reactive-session.md`](../../sessions/flows/flow-multi-trigger-reactive-session.md)

## Symptom

Saving `chain4` with **two cron trigger nodes** (`*/2 * * * *` and `1 * * * *`) was **rejected**:

> flow `chain4` has 2 cron triggers with different schedules (...); a flow has one schedule — use one
> cron trigger (or identical specs)

The user: "I've never said max one trigger. The whole design is wrong if this is the case — needs to be
like Node-RED, unlimited nodes. It's not a cron thing — what if I add a webhook or an MQTT-sub node?"

## Root cause (two independent design bugs)

A flow's reactive identity was **hoisted to the flow level**, and a run fired the **whole graph**:

1. **One trigger's worth of reactive state per flow.** The schedule/cursor lived as scalar fields on the
   `Flow` record — `flow.cron: Option<String>`, `flow.next_attempt_ts: u64` — not on the source nodes.
   `save::derive_cron_from_trigger` collapsed every `mode:cron` trigger node into that single `flow.cron`
   and **rejected** the save when two specs disagreed. There was structurally nowhere to hold two cron
   cursors — and the same wall blocks two MQTT subs or two webhooks. The reactor scanned **flows**,
   reading the one `flow.cron`.
2. **A run fired the whole DAG from every root.** `run_store::create_run` enqueued **every** indegree-0
   node, so even with N triggers, one run fired all of them at once — no "this trigger pushed a message
   down its own wires."

In Node-RED a flow is a soup of nodes with **N independent source nodes**, each emitting down only its
downstream wires on its own schedule/subscription. Reactive state belongs to the **node**, not the flow.

## Fix

**Per-node triggers.** A durable per-node cursor `flow_trigger_state:{flow}:{node}` (`trigger_store.rs`);
`react_to_flows_cron` rewritten to scan **every cron trigger node** of every enabled flow, fire each due
one independently (entry = that node), advance **that node's** cursor; run id `{flow}-cron-{node}-{ts}`.
`save` drops the collapse + rejection (`validate_cron_triggers`) — N cron triggers are valid; only a
malformed spec is rejected.

**Per-trigger (per-wire) subgraph runs.** `Flow::reachable_from(entry)` + `indegrees_within(set)`;
`create_run` takes `entry: Option<&str>` (seeds only the reachable subgraph, indegrees within it),
records `entry_node`, and `finalize_if_complete` scopes to the run's node set. `entry` is threaded to
cron / inject / boot / `flows.run {entry}`. `None` keeps the whole-graph path (manual run, resume,
subflow).

**Bonus (the original "count goes up").** A stateful `counter` builtin backed by durable node memory
(`flow_node_memory:{flow}:{node}`), incremented via a new **atomic** `lb_store::increment` (server-side
accumulate, per-key serialized — a retry can't double-add). Distinct from the pure `count` transform.

## Proof

- `flows_multi_trigger_test`: two distinct cron specs in one flow **save** (no rejection) and fire on
  independent cursors; firing trigger A runs only `{A,x}` (`entryNode` recorded), B/y untouched; the
  counter goes 1→2→3 across firings; cap-deny + ws-isolation.
- `increment_test`: 64 concurrent firings each get a unique total 1..=64 (no lost update); ws-walled.
- `flows.save` no longer rejects multiple distinct cron triggers (the exact failing case).

## Lesson

When a "reactive runtime" keeps its reactive state (schedule/cursor/arm) on the *container* instead of
the *node*, it can hold exactly one trigger — and every "add another source" request hits a wall. Put
the state on the node; let the engine fire each node's own subgraph. (And a counter that "goes up" needs
durable *node memory*, atomically mutated — not a pure transform of the current input.)
</content>
