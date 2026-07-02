# `switch` routed to BOTH branches — an `else` rule matched unconditionally instead of as a fallthrough

- **Date:** 2026-07-01
- **Area:** flows (switch edge-gating, data-nodes pack Tier C)
- **Status:** resolved

## Symptom

The Tier-C integration test `switch_fires_only_the_matched_port` failed: a `switch` routing a payload of
`10` (with rules `[{op:gt, value:5, to:["hi"]}, {op:else, to:["lo"]}]`) fired **both** `hi` and `lo`. The
`else` (fallthrough) branch `lo` ran (`outcome == "ok"`) when it should have been gated (`skipped`) —
only `hi` should run.

```
assertion `left != right` failed: unmatched branch does NOT run
  left: "ok"   (lo)
 right: "ok"
```

## Root cause

`switch::matched_targets` (`crates/host/src/flows/execute_node/switch.rs`) evaluated every rule
independently and unioned the `to` targets of all that matched. But the `else` operator in the shared
`ops::predicate::eval` **always returns true** (it is the deliberate "fallthrough" op). So an `else` rule
matched on *every* payload, and its targets fired alongside the genuinely-matched rule's — the classic
Node-RED "otherwise fires even when a case matched" bug.

## Fix

`matched_targets` now treats `else` as a **fallthrough**, not a predicate: it first evaluates all
non-`else` rules (collecting their targets, tracking `matched_any`); only if **no** concrete rule matched
does it fire the `else` rule(s). `stop_on_first` short-circuits the concrete pass, and the `else` pass is
skipped whenever any concrete rule matched. (This is a pure routing-decision change; the wire/gating
mechanism — release matched dependents, `skip_gated` the rest — was already correct.)

## Regression test (real store/caps/jobs — no mocks)

`crates/host/tests/flows_data_engine_test.rs::switch_fires_only_the_matched_port` drives a real run
through `sw → {hi, lo}`, routes `10` (>5) to `hi`, and asserts `hi == ok` **and** `lo == skipped` (the
gated branch records no `ok`). **Fail-before verified:** the pre-fix `matched_targets` fired `lo` (`ok`),
the assertion failed; with the fallthrough fix all 11 engine tests pass.

## Lesson

A "match everything" sentinel op (`else`/`otherwise`/`default`) is a **control-flow** construct, not a
predicate — evaluating it in the same independent pass as real predicates makes it fire additively.
Fallthrough must be a distinct phase that runs only when nothing else matched.
