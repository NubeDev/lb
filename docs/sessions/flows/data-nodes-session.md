# Flows — the data & JSON node pack (20 built-in nodes) (session)

- Date: 2026-07-01
- Scope: ../../scope/flows/data-nodes-scope.md
- Stage: S8+ (data plane shipped; this extends the flows built-in registry)
- Status: done

## Goal

Ship the **twenty** data/JSON built-in flow nodes end to end — Tier A (pure stateless transforms),
Tier B (durable-state), Tier C (engine-extending) — in the exact shape the existing eight built-ins
wear, resolve **every** open question in the scope, and prove it against the real store/caps/jobs.
Zero open questions left; nothing deferred silently.

## What changed

### New pure-logic crate surface (`lb-flows`)
- `crates/flows/src/ops/` — the pure transform logic (no store/bus), unit-tested in-crate:
  - `path.rs` — the shared dot-path get/set/delete (the binding walker verbatim, Risk 5 / Q4).
  - `predicate.rs` — the shared `{op,value}` evaluator (`switch`/`filter` share it, Risk 5).
  - `data.rs` — `change`/`select`/`merge`/`map`/`flatten`/`sort`/`range`/`aggregate`.
  - `template.rs` — a hand-rolled mustache-lite (`{{dot.path}}`; no templating engine, Risk 4).
  - `parse/{text,xml}.rs` — `csv`/`yaml`/`base64` + the event-driven `xml` convention (malformed → Err).
  - `sequence.rs` — `split`/`join` + the `parts` sequence contract (array-carry, Decision 15).
- `crates/flows/src/builtins/` — `builtins.rs` split into `{core,data,parse,sequence,function}.rs`
  (+ `mod.rs`), 8 spine + 20 new descriptors, each a compilable JSON-Schema 2020-12 config (Q5).
- `crates/flows/Cargo.toml` — the four parse crates (`csv`/`quick-xml`/`serde_yaml`/`base64`); rows
  added to `docs/key-stack.md`.
- `lb_flows::table::FLOW_NODE_BUFFER` — the one additive bounded-accumulator table.

### Host execution (`lb-host`)
- `crates/host/src/flows/execute_node.rs` (534 lines, over the FILE-LAYOUT limit) → `execute_node/`:
  `mod.rs` (orchestrator + router + shared helpers), `core.rs`, `sink.rs`, `subflow.rs`, `pure.rs`
  (the Tier-A wrapper over `ops::*`), `stateful.rs` (filter/unique/batch), `switch.rs` (edge gating),
  `delay.rs` (durable park).
- `crates/host/src/flows/buffer.rs` — the durable bounded accumulator (`batch_append`/`unique_seen`,
  `BATCH_MAX = 1000`, force-release; per-`{ws}:{flow}:{node}` lock).
- `run_store.rs` — `park_step` (delay suspend), `ready_one_dependent` + `skip_gated` (switch gating).
- `run.rs::flows_resume` — clears a `suspended` status (and refuses a `cancelled` run) before
  re-driving, so a `delay` park (or a `flows.suspend`) actually resumes.
- `execute_node/mod.rs::execute_one` — a `Skipped` outcome (RBE suppress) gates the subtree; a
  `switch` releases only matched dependents; a `delay` `Park` resets to `Enqueued` + suspends.

## Decisions & alternatives

All five scope **open questions RESOLVED** (see the scope doc), plus three new spine **Decisions**:

- **Q1 → Decision 14 (`switch` = edge-gating, not a wire `Outcome`).** Settle `Ok`, then release only
  matched dependents (rules carry `to:[node_ids]`) and `skip_gated` the rest. A suppressing stateful
  node reuses the seam via `Skipped`. *Rejected:* a port-labelled edge model; a null/skip sentinel.
- **Q2 → Decision 15 (`split`/`join` = array-carry).** One settle carries the array + a `parts`
  descriptor; `join` recombines from the carried `parts`. Collapses split/join to pure array
  transforms; per-element work is `map`/`sort`/`aggregate`. *Rejected:* per-message fan-out (the
  Decision-9 fan-out storm).
- **Timer → Decision 16 (`delay` parks on the resume seam).** Durable release instant + suspend/resume
  (the subflow-park seam), never a `tokio::sleep`. Survives restart. *Rejected:* an in-memory sleep.
- **Q3:** capped `flow_node_buffer` (`BATCH_MAX`), **force-release** on overflow; time-window batching
  is an explicit deferral to the reactor (count mode shipped) — a named non-goal, not a silent gap.
- **Q4:** exactly the existing field-path walker (`ops::path`), no wildcards.
- **Q5:** yes — split `builtins/` and `execute_node/` by category/verb (all files < 400 lines).

## Tests

Mandatory categories: **capability-deny** (`capability_deny_run_without_flows_run_cap_executes_no_node`)
and **workspace-isolation** (`workspace_isolation_batch_accumulator` — a `batch` accumulator in ws1 is
invisible to the same flow in ws2). Plus Tier A table-driven (15), Tier B two-firing + cross-run
persistence (filter RBE, batch count boundary, unique stream/array), Tier C (switch gating,
split→join round-trip, split→map→join, delay park+resume, rate-limit).

Green output:

```
# lb-flows unit (ops + builtins + model + descriptor + registry)
test result: ok. 78 passed; 0 failed; 0 ignored

# Tier A integration (crates/host/tests/flows_data_nodes_test.rs)
test result: ok. 15 passed; 0 failed; 0 ignored

# Tier B + Tier C integration (crates/host/tests/flows_data_engine_test.rs)
test result: ok. 11 passed; 0 failed; 0 ignored

# existing flows host suites (no regressions; flows_nodes_test updated to 28 builtins)
flows_run_test:            5 passed
flows_sink_test:           4 passed
flows_nodes_test:         25 passed
flows_plc_reliability:    12 passed
flows_triggers_test:       4 passed
flows_runtime_control:    17 passed
```

Full-workspace `cargo build --workspace && cargo test --workspace && cargo fmt` — see the final run
summary appended below.

## Debugging

- [flows/switch-else-branch-fires-unconditionally.md](../../debugging/flows/switch-else-branch-fires-unconditionally.md)
  — the `else` fallthrough op matched additively (it always returns true), so a `switch` fired both the
  matched and the `else` branch. Fixed by making `else` a distinct fallthrough phase; regression
  `switch_fires_only_the_matched_port` fails-before/passes-after.

## Public / scope updates

- Promoted the shipped surface into `public/flows/flows.md` (the 20-node pack + the 3 new Decisions).
- `data-nodes-scope.md` — all five open questions marked RESOLVED, with the inline decisions recorded.
- `flows-scope.md` — Decisions 14/15/16 added.
- `docs/key-stack.md` — the parse-crate row. `STATUS.md` — the slice marked shipped.

## Dead ends / surprises

- Array-carry (Decision 15) means a **scalar** node (`range`/`filter`) between `split` and `join`
  receives the whole array, not an element — per-element scalar transforms are NOT the array-carry
  model; use array-native `map`. Documented in the scope; the worked IoT example's per-reading `range`
  is illustrative of the *rejected* per-message model.

## Follow-ups

- A timer-reactor that auto-resumes an elapsed `delay` park (v1 resume is operator/reactor-driven).
- Time-window `batch` (the reactor-driven sibling of count mode).
- Canvas palette rendering of the new categories (Data/Parse/Sequence/Function) — UI, not this slice.
