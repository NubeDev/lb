# The rule `insight` handle's `route:false` no-op had no flag to honor — `route` never reached the cage

- **Area:** rules
- **Symptom:** Building the `insight.raise`/`ack`/`close` rhai handle (rule-raises-insight-scope), the
  `route:false` suppression decision ("a panel repaint raises nothing") assumed the run's `route` flag
  was available at handle-construction time. It was **not**: a naively-built handle would have had no
  `route` to check and would have raised a durable insight + fired the notify ladder on **every 30 s
  dashboard repaint** — the exact spam slice 2's `route:false` promise forbids.
- **Status:** resolved (caught before shipping, by verifying the plumbing first)
- **Date:** 2026-07-09

## What was observed (the load-bearing verification)

The scope flagged this as risk #1 ("the `route:false` thread must actually reach the handle … verify
slice-2 plumbing first"). Grepping `route` across `rust/crates/rules/` and `rust/crates/host/src/rules/`
showed it living **only** in the host's post-run routing:

- `host/src/rules/run.rs` — `rules_run(..., route: bool)` uses `route` solely at
  `if route { route_alerts(...).await? }`, **after** `engine.run(...)`.
- `host/src/rules/mod.rs` — parses `route` from the MCP input and threads it into `rules_run`/`rules_eval`.
- `crates/rules/` — the `RuleEngine`, `verbs::register(...)`, and `RunHandles` **never received `route`**.

`alert()` suppression is entirely host-side: findings are collected in the cage, then the host chooses
not to route them. The cage never knew a run was read-only. So an `insight` handle built like
`ChannelHandle` (seam + meter + `now`) had no `route` field to short-circuit on — the "no-op on a panel
run" behavior was **unbuildable** without new plumbing.

## Root cause

`route` was a post-run host concern in slice 2 (it only gated the alert fan-out, which is host-side), so
it was never plumbed into the engine. Insights differ: the suppression must happen **inside** the cage
because the write is initiated **by the rule body** (`insight.raise(...)`), not routed after the fact
from a collected finding. An `insight.raise` is a stronger effect than `alert()` (durable record **plus**
notify fan-out); dedup collapses the record but not the `count` bump / occurrence append / notify re-fire,
so the only honest fix is to skip the call — which requires the cage to know `route`.

## Fix

Thread `route` into the cage, minimally and symmetrically with `now`:

- `RuleEngine` gained a `route: bool` field, defaulting to `true` via `new` and set by a
  `with_route(route)` builder (so all existing `RuleEngine::new(...)` callers/tests stay unchanged —
  backward-compatible).
- `verbs::register(...)` gained `route` + `origin_ref` params; `engine.run` passes `self.route` +
  `rule.name` (the origin ref).
- `RunHandles` gained an `insight: InsightHandle` constructed with the `route` flag; the handle
  short-circuits **before** charging the meter when `route == false` (charges nothing, logs an honest
  `insight.<verb> skipped: read-only panel run` line, returns an echoed id for `raise`).
- `host/src/rules/run.rs` — `RuleEngine::new(...).with_route(route)`.
- `HostMessagingSeam` needed **no** change — it is a generic `call_tool(tool, …)` chokepoint (rule 10)
  and already gates `insight.*` by `mcp:<tool>:call`.

## Regression

- `crates/rules/tests/insight_test.rs::route_false_no_ops_every_method_and_logs_the_skip` +
  `route_true_writes_where_route_false_did_not` — the handle-level proof (no dispatch, no charge, three
  skip log lines at `route:false`; the same body dispatches at `route:true`).
- `crates/host/tests/rules_test.rs::route_false_run_raises_no_insight` — the end-to-end proof against a
  real `mem://` store: `route:false` leaves the record count at 0 (run still succeeds, findings return);
  the `route:true` contrast writes one real record.

## Lesson

A flag that "already exists" for one effect may not reach the layer a new effect needs it in. `route`
gated a **host-side** fan-out (alerts); an insight raise is a **cage-initiated** write, so the same flag
had to be plumbed one layer deeper. Verify where a load-bearing flag actually lives **before** building
the behavior that depends on it — a handle that silently has no flag to honor looks done and ships broken.
