# Session — the `flipflop` source node (a self-driving boolean oscillator)

Scope: [`docs/scope/flows/flip-flop-node-scope.md`](../../scope/flows/flip-flop-node-scope.md).
Branch: `master` (per user: stay on this branch).

## The ask

A node with **no input and one output** that flips `true`/`false` on a user-set interval — "10 sec
true / 10 sec false". The user opened the Flows palette, searched "flip", and saw *No nodes match* —
because the scope doc existed but nothing was registered. This session shipped the real node.

## The key design call

The palette is populated from the live `flows.nodes` registry (`builtin_descriptors()`), **not** from
docs — so a scope doc alone never appears. And a no-input/one-output/self-timed node is structurally a
**trigger** (`NodeKind::Trigger`, empty input ports, the reactor fires the run), *not* a data-pack
transform (those are pure one-input→one-output functions). It's a **stateful trigger**: the durable
cursor holds both the clock AND the last value, so both survive restart.

## What shipped

**Registration → palette:**
- `crates/flows/src/builtins/core.rs` — the `flipflop` `NodeDescriptor` (category `Flow`, icon
  `toggle-left`, no input, `payload`/`topic` out; config `period_secs`/`start`/`topic`). This alone is
  what makes it show in the palette search.
- `crates/flows/src/builtins/mod.rs` — added to the `EXPECTED` guard (9 spine + 20 pack = 29) + a
  `flipflop_is_a_no_input_trigger` test; slice bounds for the pack-transform test moved `8..`→`9..`.

**The clock + value (host):**
- `crates/host/src/flows/record.rs` — `FlowTriggerState` gained `period_secs: Option<u64>` (interval
  marker; a changed period re-seeds the cursor exactly as a changed `cron` does) and `flop:
  Option<bool>` (last emitted value; `None` → emit `start`).
- `crates/host/src/flows/trigger_store.rs` — `flipflop_triggers(flow)` (mirror of `cron_triggers`),
  period clamped to `≥ 1s`.
- `crates/host/src/flows/react_interval.rs` — **new file**, the interval sibling of `react_cron`:
  scan enabled flows → for each due flip-flop, read the cursor, emit `!flop` (or `start`), fire the run
  from that node (`Some(node_id)` → only its subgraph), persist the new value + advance
  `next = scheduled_ts + period_secs`. Deterministic `flipflop_run_id`. Value passed into the run as a
  param under the node id — the existing `core::trigger` execute leg reads it and emits `{payload,
  topic}` (added `"flipflop"` to that match arm in `execute_node/mod.rs`).
- `crates/host/src/flows/reactor_loop.rs` — the production tick now calls `react_to_flows_interval`
  alongside `react_to_flows_cron` each pass.
- Re-exported `react_to_flows_interval` + `flipflop_run_id` from `flows/mod.rs` and `lib.rs`.

**Unit chosen — `period_secs`, not `period_ms`:** the reactor clock is wall-clock **seconds**
(`react_cron`'s `now = as_secs()`) and scans every few seconds, so sub-second is impossible. Seconds is
the honest unit and matches the ask.

## Tests (all green) — `crates/host/tests/flows_flipflop_test.rs`

Real store (`mem://`), real `lb-jobs`, real caps, injected clock. 7 tests:
- `flipflop_oscillates_true_false_true` — three due firings emit `true → false → true`; cursor
  advances by period each time (100→110→120→130).
- `flipflop_start_false_emits_false_first` — `start=false` inverts the sequence.
- `flipflop_value_survives_a_store_round_trip` — **restart parity**: after firing to `false`, the next
  pass reads the persisted side and flips to `true` (not reset to `start`).
- `flipflop_re_scan_is_idempotent_no_double_flip` — a re-scan at the same `now` is a no-op; the
  advanced cursor is the idempotency guard (same as cron).
- `flipflop_disabled_flow_never_fires` — an `enabled=false` flow never fires and primes no cursor.
- `flipflop_workspace_isolation` (**mandatory**) — a ws-B pass never sees/fires a ws-A flip-flop; ws-B
  has no cursor for the same flow id (the `{ws}:` prefix holds).
- `flipflop_capability_deny_no_run_no_state` (**mandatory**) — no NEW cap; a caller lacking
  `mcp:flows.run:call` is `Denied` at the bridge and no run record is written.

Also re-ran `flows_triggers_test` (10) + `lb-flows` builtins (8) — all green. `cargo build --workspace`
clean; `cargo clippy -p lb-flows -p lb-host` produced **no** warnings from the new files.

## Two findings surfaced by the tests (and fixed)

1. **Idempotency is the cursor advance, not job-load.** Like cron, after firing at instant T the cursor
   advances to T+period, so a re-scan at the same `now` early-returns (`scheduled_ts > now`) and never
   double-fires. The `lb_jobs::load` guard only matters on a genuine at-least-once redelivery *before*
   the advance persists. The test asserts the real behaviour (re-scan → `fired:0`, value unchanged).
2. **The reactor bypasses the caller cap-gate — by design.** Internal `flows_run` does not cap-check
   the passed principal (that principal is for the *tool nodes'* own `caller ∩ grant`); the `flows.run`
   gate lives at the MCP bridge, which the reactor (running under the node's system principal)
   bypasses — identical to cron. The deny test therefore drives `flows.run` through the bridge without
   the cap, matching the honest boundary.

## Docs

- Scope marked **SHIPPED** with an "As built" delta (`period_secs`; value in the cursor `flop` field,
  resolving Open Q1).
- README table row added under `flows/`.
- This session log.

## Follow-ups (deferred, named — not silent gaps)

- Symmetric duty only (equal true/false time); asymmetric `on_secs`/`off_secs` deferred.
- Effective resolution floor = the reactor scan cadence; `period_secs` min clamped to 1s.
- Promote to `docs/public/flows/` when the flows public doc is next refreshed.
