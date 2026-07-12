# Matched switch into a multi-wire port hangs the run (never reaches terminal)

- Area: flows
- Status: resolved
- First seen: 2026-07-12 (latent since the switch edge-gating shipped; promoted to mainline by the
  `flow-plain-wiring` any-default flip)
- Resolved: 2026-07-12
- Session: ../../sessions/flows/flow-plain-wiring-session.md
- Regression test: rust/crates/host/tests/flows_run_test.rs
  (`matched_switch_into_a_multi_wire_any_port_reaches_terminal`, plus the gated and
  explicit-all-barrier siblings)

## Symptom

A flow with a matched `switch` plus plain wires into the same downstream node never reaches a
terminal status. `flows.runs.get` shows the downstream node's slot stuck `pending`; `await_terminal`
in tests times out; a live run sits "running" forever.

## Reproduce

`a → w`, `b → w`, `src → switch → w` where the switch rule matches. Run the flow. Under the
universal-`any` default (flow-plain-wiring), `w`'s two plain wires each mint a per-message firing —
but the matched switch releases `w` through the barrier path, seeding a `(w, fctx)` slot `Pending`
with indegree 3 (all of `w`'s wired upstreams). The two any-firings run under their own minted slot
ids and never decrement that barrier slot, so it stays `Pending` and `finalize_if_complete` never
fires.

## Investigation

Named by the peer review of `flow-plain-wiring-scope.md` before implementation: the switch's
`release_matched` called `ready_one_dependent` → `touch_barrier_slot` **unconditionally**
(`switch.rs` → `run_store.rs`), ignoring the dependent port's join policy. Latent even before the
flip for a multi-wire `any` sink downstream of a switch; the flip made it the mainline topology.
Fail-before verified mechanically: the new regression test was run against the pre-fix barrier
release (temporarily reverted) and hung until the bounded poll panicked.

## Root cause

Two release paths existed with different policy awareness: `release_dependents` consulted
`join_of(port)` per dependent, but the switch's matched release had its own barrier-only shortcut.
Any future caller of the shortcut would re-introduce the hang.

## Fix

One policy-aware release seam: `run_store::release_one_dependent` (the per-dependent body of
`release_dependents`, now public within the crate). The switch's matched release calls it, so an
`any` dependent port gets a normal minted/propagated firing (`triggered_by` = the switch, its routed
envelope auto-wired) and only an explicit-`all` port takes the barrier decrement.
`ready_one_dependent` (the barrier-only shortcut) is deleted so the class cannot recur. The gated
(unmatched) side is unchanged: the `(dep, fctx)` slot settles `Skipped` — one fewer firing, run still
terminal.

## Verification

`matched_switch_into_a_multi_wire_any_port_reaches_terminal` — run reaches `success`, `w` fires
three times (`w#a`, `w#b`, `w#sw`) with payloads `{1, 2, 5}`; hangs before the fix.
`gated_switch_wire_into_a_multi_wire_port_settles_skipped` and
`matched_switch_into_an_explicit_all_port_takes_the_barrier` pin the other two sides of the seam.

## Prevention

The regression tests above + the single-seam structure (there is no non-policy-aware release
function left to call).
