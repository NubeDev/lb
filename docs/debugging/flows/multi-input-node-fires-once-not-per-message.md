# A multi-input flow node fires once, not once-per-message (Node-RED OR unreachable)

- Date: 2026-07-09
- Area: flows
- Scope: ../../scope/flows/flow-input-ports-scope.md (Slice 2)
- Status: fixed (the runtime seam — Slice 2 of the input-ports scope)

## Symptom

A flow node with multiple wires into a single input fired **once** (an AND barrier that joined every
upstream), never **once per upstream** (Node-RED's fire-per-message OR). Three `mqtt` sources wired
into one `debug` node printed **once**, not three times. The familiar Node-RED convergence behaviour
was simply unreachable within a single run. Worse: which semantics you got was **not a decision the
flow author made** — it fell out of `indegree` counting plus a save-time lint (`≥2 inputs ⇒ must bind
payload`), and the OR case was structurally inexpressible.

## Root cause (read end-to-end in the code)

Two coupled defects in the engine, both named by spine Decision 14's deferred port-labelled-edge
model (which this slice executes):

1. **Edges were node-id `needs`, not port-targeted wires.** The descriptor *spoke* of ports but the
   runtime edge was a bare `needs: [node_id]` with **no target-port label**. So the engine could not
   tell "two wires into one input" (Node-RED OR) from "two different inputs to join" (AND). It guessed:
   ≥2 upstreams ⇒ AND barrier.

2. **A node fired exactly once per run.** The frontier claimed each node under a single key
   `flow_step_output:{ws}:{run}:{node}` — **one settle per node per run**. There was no seam for
   "Z fired once for A's message, then again for B's message." OR-per-message was not expressible.

The fix needed both: port-labelled edges (so the author declares per-port **join policy** — `all`
barrier vs `any` funnel) AND a per-firing identity carried down the run so multiplicity survives past
the funnel. A naive depth-1 suffix (`{node}#{upstream}`) was the trap: it disambiguates **only at the
funnel** and breaks one hop downstream (a node with a single wire from the funnel has one slot, can
settle only once, and `${steps.funnel.payload}` is ambiguous across its firings).

## Fix (flow-input-ports-scope Slice 2)

**The firing context (`fctx`)** — a per-message identity carried down the run, the load-bearing seam
(`rust/crates/flows/src/firing_context.rs`):

- An `any` port releases **once per settled upstream**, minting a firing id stamped into an **additive
  envelope field `fctx`** (`mint(node, upstream, parent_fctx)`; nested funnels extend it:
  `link-in#mqtt-a` → `link-in#mqtt-a·funnel2#w`). Deterministic per `(node, upstream, parent)` so a
  redelivered upstream re-mints the SAME id ⇒ exactly-once per firing.
- Every step-output claim key is now keyed by **`(node, fctx)`**: `{run}:{node}` when `fctx` is empty
  (the all-`all` common case ⇒ **byte-for-byte today's key**), `{run}:{node}@{fctx}` otherwise.
- `${steps.X}` resolves against the upstream settle **carrying the same `fctx`**, so a node one hop
  past a funnel reads *its* firing's message for free — multiplicity propagates, no per-event fan-out.

**Engine rewrite** (`rust/crates/host/src/flows/run_store.rs` + `coordinator.rs` +
`execute_node/mod.rs`):

- **Frontier-only seeding + dynamic release.** A run now seeds only indegree-0 nodes; every other
  `(node, fctx)` slot is minted by `release_dependents` as upstreams settle — a barrier slot on first
  touch (with its in-subgraph indegree, decremented per same-`fctx` upstream settle, Enqueued at 0),
  an `any` firing directly Enqueued with `triggered_by` + `parent_fctx`. For all-`all` this is
  byte-identical (every slot at `fctx=""`, key `{run}:{node}`).
- **Run-terminal counts slots, not nodes.** `finalize_if_complete` scans the run's step records: every
  minted slot `Done` AND every subgraph node has ≥1 slot. A gated-skip on a never-seeded dependent
  now CREATES a `Skipped` slot (the bug that hung the first run of `switch_fires_only_the_matched_port`
  after the rewrite — a `switch`-gated dependent had no slot, so finalize waited forever).
- **The `any`-firing auto-wires** its single triggering upstream (the one message), carrying that
  upstream's non-`payload` fields forward; a barrier resolves `${steps.X}` against same-`fctx` settles
  (today's logic).
- **Per-kind default policy.** `sink`-kind ports (incl. `debug`) default to `any`; everything else to
  `all`. The save join lint is now **per-port policy-aware** (an `any` port with N wires is valid —
  the funnel — ; an `all` port with ≥2 wires must still bind `payload`).

## Regression test (fail-before / pass-after)

`flows_run_test::any_funnel_fires_once_per_upstream` — three `rhai` sources (distinct payloads 1/2/3)
wired into one `debug` node. **Fail-before** (the old engine): `debug` settles **once** (a join
barrier). **Pass-after**: the run reaches `success` with **three** `debug` settles, each under a
distinct `fctx` (`dbg#a`/`dbg#b`/`dbg#c`), each carrying its **own** upstream's payload (1, 2, 3) —
Node-RED's fire-per-message OR, in one durable run, exactly-once per firing.

Plus the byte-identical guard `all_join_barrier_settles_once_at_empty_fctx` (a 2-upstream `all` join
settles once at `fctx=""`, no `@fctx` suffix) + `workspace_isolation_any_funnel_step_keys` (a ws-B
caller can't read a ws-A funnel's per-firing `@{fctx}` slots — the mandatory isolation category
re-asserted for the new key shape).

## What's deferred to Slice 3 (named, not silent)

The **propagate-one-hop-past-the-funnel** end-to-end test (the scope's THE-seam fail-before for a
naive depth-1 suffix) needs a non-sink `any` node so the funnel has a downstream — i.e. the `link-in`
built-in pair. Slice 2's only `any` nodes are terminal sinks (`debug`), so the topology is not yet
expressible; the `fctx` machinery that handles propagation (mint extending the parent, same-`fctx`
resolution) is built and unit-tested (`firing_context::mint_extends_a_non_empty_parent_for_nested_funnels`),
and the test lands with `link-in` (Slice 3) — where `link-in` (any, 3 wires) → transform `W` must
settle **three** times, each reading its own firing's message. Per-firing cap-deny + outbox-dedup-per-
firing ride the same topology.

## Lesson

A "join vs funnel" semantics inferred from `indegree` + a lint is a side effect dressed as behaviour —
the safe (join) path is the *only* path and the familiar (funnel) path is unreachable, and the author
never chose. Making the input the typed, first-class unit on two axes (port-labelled edges + a declared
per-port policy) turns the guess into a decision — and the multiplicity must propagate via a carried
identity (`fctx`), not a depth-1 suffix, or it works at the funnel and silently collapses one hop down.
