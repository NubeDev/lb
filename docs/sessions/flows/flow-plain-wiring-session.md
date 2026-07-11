# Session — flows: plain wiring (remove link pair, any-by-default per-message firing)

- Status: in-progress
- Date: 2026-07-12
- Scope: [../../scope/flows/flow-plain-wiring-scope.md](../../scope/flows/flow-plain-wiring-scope.md)
- Branch: `flow-plain-wiring`

## The ask

Remove the `link-out`/`link-in` built-in pair and flip the default join policy to `any`
for every node kind, so plain wiring — N wires onto a port, one firing per arriving
message — is the whole story (the Node-RED model). Fix the latent `switch`
matched-release hang (barrier-path release ignores the dependent port's policy), widen
`${steps.X}` binding resolution to the firing lineage, add the cross-branch save lint,
and add a run-load unknown-kind guard for already-armed persisted flows.

## Plan (slices)

1. Flip + switch-fix + lineage resolution, with tests.
2. Link removal + dead-code sweep + run-load guard, with tests.
3. UI mirror + gateway tests.
4. Docs scrub + promotion.

## Log

- Read HOW-TO-CODE, FILE-LAYOUT, the scope, the input-ports scope, testing/debugging
  scopes. Branched `flow-plain-wiring` off master.

## Decisions made in-session

(recorded as they happen)

## Test evidence

(green output pasted at the end)
