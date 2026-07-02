# Flows scope — the `flipflop` source node (a self-driving boolean oscillator)

Status: **SHIPPED** (built + green, this session). Descriptor + interval reactor + 7 host tests.
Promotes to `public/flows/flows.md` next. Session log:
[`docs/sessions/flows/flipflop-node-session.md`](../../sessions/flows/flipflop-node-session.md).

> Read the spine first: [`flows-scope.md`](./flows-scope.md) owns the canonical **Decisions (v1)**
> this doc references by number; [`node-descriptor-scope.md`](./node-descriptor-scope.md) owns the
> **descriptor shape** this node wears; [`triggers-lifecycle-scope.md`](./triggers-lifecycle-scope.md)
> owns the durable **cron clock** it rides. This is a *content* ask — **one new built-in descriptor** —
> plus the one small reactor arm that lets a clock-fired source alternate its value.

## What it is

A **source** node with **no input port and one output port** (`payload`). On a user-chosen interval
it emits a boolean that **flips** each firing: `true, false, true, false, …`. The user configures how
often it changes; nothing feeds it. It is the flow equivalent of a hardware square-wave / PLC blink
bit — the smallest possible "make something happen on a clock" node, and a first-class demo/test
source.

Node-RED expresses this with an *inject* wired into a *change*/*rbe* + a flag; we ship it as **one
node** because "oscillate a boolean on a timer" is a single, common, self-contained intent.

## Why it is a Trigger, not a data-pack node

The [data-nodes pack](./data-nodes-scope.md) is explicitly **one input → one output, pure functions of
their input**. This node has **no input**: its value comes from a *clock* + *its own last value*, not
from an upstream payload. Structurally that is the `trigger` family (`NodeKind::Trigger`, empty input
ports, fires the run itself), **not** a Transform. It is a *stateful trigger*: the durable cron clock
(already shipped — [`react_cron.rs`](../../../rust/crates/host/src/flows/react_cron.rs)) says *when*,
and the Decision-5 `flow_node_state` last-value record says *which side* to emit next.

*Rejected:* a `toggle` Transform in the data pack that flips its input boolean. That is a different,
also-useful node (flip whatever arrives), but it is **not** what was asked — the ask is a source with
**no input, one output** that changes on its own. Keeping them separate keeps each honest; a `toggle`
transform can land later in the data pack if a caller needs it.

## The descriptor (the whole flow-crate surface)

One `NodeDescriptor` in [`builtins/core.rs`](../../../rust/crates/flows/src/builtins/core.rs) (it lives
with `trigger`/`counter` — the other clock/stateful spine nodes — not in the pure-transform packs):

> **As built (this session):** the unit shipped as **`period_secs`** (seconds), not `period_ms`. The
> reactor's clock is wall-clock **seconds** (`react_cron`'s `now` is `as_secs()`), and the reactor scans
> every few seconds — sub-second resolution is impossible, so seconds is the honest unit and matches the
> ask ("10 sec true / 10 sec false"). The last value also ships **in the trigger cursor record**
> (`FlowTriggerState.flop`), not in `flow_node_state` — so clock + value are one durable record that
> moves together (resolves Open Q1 in favour of the explicit field). Everything else below is as shipped.

```rust
NodeDescriptor::new("flipflop", NodeKind::Trigger, "")
    .with_title("Flip-flop (oscillator)")
    .with_category("Flow")
    .with_icon("toggle-left")
    .with_ports(vec![], vec!["payload".into(), "topic".into()]) // NO input; envelope out
    .with_config(1, json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "period_secs": {"type": "integer", "minimum": 1, "default": 10,
                "description": "how long each value is held before it flips, in seconds (10 → 10s true / 10s false)"},
            "start": {"type": "boolean", "default": true,
                "description": "the value emitted on the FIRST firing"},
            "topic": {"type": "string",
                "description": "topic stamped on the firing envelope (D6)"}
        }
    }))
```

Notes:
- **`period_secs`, not `cron`.** A sub-minute square wave doesn't fit a 5-field cron. This is the one
  place flip-flop *diverges* from the plain `cron` trigger — it drives an **interval** cursor, so the
  clock arm (below) computes `next = scheduled_ts + period_secs` instead of `next_after(cron, now)`.
  Everything else about the durable cursor (deterministic run id, fire-once-then-skip, ws-walled scan,
  init-on-schedule-change) is **reused verbatim** from `react_cron`.
- **`start`** sets the first value; the flip is applied *after* emitting, so firing N emits
  `start XOR (N odd)`.

## The one host arm (reuse the cron reactor)

`flipflop` is a new **trigger kind** the reactor scans alongside `cron`. In
[`react_cron.rs`](../../../rust/crates/host/src/flows/react_cron.rs) (or a sibling `react_interval.rs`
if it grows past its file budget, per FILE-LAYOUT):

1. **Scan** enabled flows for `flipflop` trigger nodes (mirror `cron_triggers(flow)` →
   `flipflop_triggers(flow)`), each with its own `period_ms` cursor.
2. **Due?** `next_attempt_ts ≤ now` → fire; init/advance the cursor with `next = now + period_ms`
   (the *only* substitution for `next_after`).
3. **Value from durable state.** Read this node's `flow_node_state:{flow}:{node}` last value; emit
   `!last` (or `start` on first sight). Fire the run from this node (`Some(node_id)` → only its
   subgraph runs, exactly as cron does), stamping `{ "value": v }` as the trigger's output payload.
4. **Persist** the new value into `flow_node_state` (Decision 5, the record `counter`/`filter` already
   use) **and** advance the cursor. Both in the same pass — the value and the clock move together.

No new `Outcome`, no new storage table, no new capability. The value survives restart because it lives
in `flow_node_state`, and the clock survives restart because it lives in the trigger cursor —
**stateless reactor over durable state** (CLAUDE rule 4: never an in-process `sleep`/timer).

## How it fits the core

- **Tenancy / isolation:** the scan is ws-scoped (the flow directory is), and the value record is keyed
  `{ws}:{flow}:{node}` — a ws-B reactor never fires or reads a ws-A flip-flop. The mandatory
  workspace-isolation test seeds a flip-flop's last value in ws 1 and asserts the same flow id in ws 2
  starts fresh from `start`.
- **Capabilities:** **none added.** Like `cron`, it fires a `flows.run` under the flow owner's
  principal — gated by the existing `mcp:flows.run:call`. The deny path is the existing "no run cap →
  the run never starts". No node here dispatches an external tool.
- **Placement:** either. It runs wherever the run owner runs (Decision 10) — no `if cloud {…}`.
- **Data (SurrealDB):** reuses `flow_node_state` (value) + the trigger cursor (clock). **No new table.**
  State only, never motion (rule 3).
- **Bus (Zenoh):** none new — its settle rides the existing per-node settle feed
  ([`flow-runtime-control-scope.md`](./flow-runtime-control-scope.md)).
- **Secrets:** none.

## Example flow

*A blink source driving a channel indicator* — the smallest useful flow, and a live-canvas demo:

1. **`flipflop`** (`period_ms=1000`, `start=true`) emits `true` then `false` every second, from the
   durable clock — no upstream node.
2. A **`sink`** (`target=channel`, `name=blink`) writes each value; the canvas indicator toggles once a
   second with nothing else wired. Restart the node mid-run: it resumes on the *next* interval with the
   *next* value (both durable), not from `start`.

## Testing plan

Per [`scope/testing/testing-scope.md`](../testing/testing-scope.md).

- **Capability-deny (mandatory):** a flow with a `flipflop` whose owner lacks `mcp:flows.run:call`
  never fires — assert no `flow_node_state` value and no cursor advance on deny. (No *new* cap — state
  that.)
- **Workspace-isolation (mandatory):** seed a `flipflop` last value `false` in ws 1; run the same flow
  id in ws 2 and assert its first firing emits `start` (empty state — the `{ws}:` prefix holds). Real
  store (`mem://`), real records — no fakes (CLAUDE §9).
- **Oscillation unit (host, `crates/host/tests/`):** drive the reactor over an **injected clock**
  (never wall-clock, per `react_cron`'s determinism rule): N passes at `period_ms` spacing emit
  `start, !start, start, …`; a pass *before* `next_attempt_ts` fires nothing (no double-fire); the
  deterministic run id is stable per `(flow, node, instant)`.
- **Restart parity:** fire twice, round-trip the store, fire again → the value continues from the
  persisted side (not `start`) and the cursor resumes on the next interval, not immediately.
- **Regression:** any bug → `docs/debugging/flows/<symptom>.md` + a regression test
  (`scope/debugging/debugging-scope.md`).

## Open questions

1. **Interval cursor vs. cron cursor — same record?** Proposed: reuse `FlowTriggerState`, storing
   `next_attempt_ts` as `now + period_ms` and leaving `cron: None` (an interval marker). If that
   overloads the record uncomfortably, add an explicit `period_ms: Option<u64>` field to
   `FlowTriggerState` — decide when writing the arm. Either way, **one** cursor record per trigger node.
2. **Duty cycle.** v1 is a symmetric square wave (equal true/false time). Asymmetric duty (`on_ms` /
   `off_ms`) is deferred — a named non-goal, not a silent gap. Add it only when a caller needs it.
3. **Reactor cadence floor.** `period_ms` can be finer than the reactor's own scan interval; the node
   can only flip as fast as the reactor scans. Document the effective floor (= scan cadence) and clamp
   `period_ms`'s `minimum` to it rather than promising sub-scan resolution.

## Related

- **Spine & contract:** [`flows-scope.md`](./flows-scope.md) (Decisions 5, 6, 10),
  [`node-descriptor-scope.md`](./node-descriptor-scope.md) (the descriptor shape),
  [`triggers-lifecycle-scope.md`](./triggers-lifecycle-scope.md) (the trigger lifecycle it joins).
- **Sibling content:** [`data-nodes-scope.md`](./data-nodes-scope.md) (why an input-driven `toggle`
  transform, if ever wanted, belongs *there* and not here).
- **Code:** [`rust/crates/flows/src/builtins/core.rs`](../../../rust/crates/flows/src/builtins/core.rs)
  (the descriptor), [`rust/crates/host/src/flows/react_cron.rs`](../../../rust/crates/host/src/flows/react_cron.rs)
  (the clock arm to extend), [`flows/execute_node/core.rs`](../../../rust/crates/host/src/flows/execute_node/core.rs)
  (the trigger execute arm).
- **Platform:** README `§6.5` (flows/rules surface).
</content>
</invoke>
