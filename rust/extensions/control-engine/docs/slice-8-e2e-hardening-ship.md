# Slice 8 — E2E, hardening, ship

Status: scope slice (S8, last). Depends on: all prior slices. Parent:
`control-engine-scope.md`.

Close the loop: the cross-cutting invariant tests, the measurements the parent scope
deferred, the agent-facing skill, and the docs promotion that marks the scope shipped.

## Deliverables

- **Core-ignorance regression test** (`ce_core_ignorance_test`, the `chains_retired`
  prove-absence pattern): grep the host/core crates (`rust/crates/host`,
  `rust/crates/mcp`, `rust/crates/caps`, `rust/role/gateway`, the UI shell) and assert
  zero live references to `control-engine` / `ce_appliance` / `ce.` tool names outside
  `rust/extensions/control-engine/`, `packages/ce-wiresheet`, and docs. CI fails if a
  CE string leaks into core. (Write it here, once the surface is complete, so it
  guards the final shape.)
- **Latency measurement** (the parent scope's open question): time `ce.patch` and
  `ce.tree` local vs routed (two-node harness + real engine where available); record
  numbers in the session doc. Decision rule: routed `ce.patch` p50 > ~150ms →
  implement client-side coalescing of rapid patches in `BridgeTransport` (debounce
  per prop); otherwise ship without.
- **COV backpressure check:** a fake emitting frames at an aggressive tick — assert
  the bus/SSE path degrades by dropping to latest (fire-and-forget), not by unbounded
  buffering.
- **Full-suite pass:** `cargo test --workspace`, `cd ui && pnpm test`,
  `pnpm test:gateway`, plus one recorded real-engine run of the S5 scripted flow +
  S6 watch.
- **`skills/control-engine/SKILL.md`** — the agent/CLI how-to, grounded in a live
  run (per SCOPE-WRITTING): register an appliance, read the tree, add two nodes,
  wire them, override, watch. The `ce.*` surface exists so agents can drive CE;
  this is its manual.
- **Docs promotion:** `docs/public/control-engine/control-engine.md` (what shipped:
  the verb table, the frame contract, the appliance model, the caps), scope open
  questions resolved or moved to the deferred list, `docs/STATUS.md` updated,
  `docs/debugging/control-engine/` entries for anything that broke along the way.

## Explicit deferred list (carried out of v1, one place)

- `ce.remove-edge`, `ce.restore` (consumes S5's returned `DeletedItems`), `ce.copy`,
  `ce.bulk`, `ce.set-layout` (first in line — unblocks drag persistence, S7 gap).
- Graph import as an `lb-jobs` job (bounded `ce.bulk` stays synchronous; imports don't).
- Presence + per-actor undo over the bridge (LB principal → CE actor mapping).
- Must-deliver command variant via the outbox ("apply when the appliance returns").
- `ce.watch` migration off the series-bridge fallback, if S6 shipped on it.

## Exit gate (= the scope's exit)

All suites green including the core-ignorance test; the skill doc verified against a
live run; `public/control-engine/` promoted; STATUS row moved to shipped.
