# Session — Persist flow node canvas positions

**Report:** "node position doesn't seem to be persisted — something seems wrong/buggy."

## Diagnosis

Not a regression — a design gap. The canvas computed each node's position from its
**array index** on every load (`layout(i)` in `ui/src/features/flows/flowGraph.ts`) and
the save path (`nodesToFlowNodes`) dropped `position` entirely. The record model
(`FlowNode` / Rust `Node`) had no geometry field. So a dragged node snapped back to the
grid on reload — exactly the "not persisted" behavior.

## Fix — geometry on the node model

Chosen (with the user) over client-only localStorage: positions save with the flow, so
the layout is shared across users/devices.

- **Rust** `rust/crates/flows/src/model.rs` — added `Node.position: Option<Position>`
  (`{x: f64, y: f64}`), `#[serde(default, skip_serializing_if = "Option::is_none")]`.
  Additive serde default → pre-geometry flows still load, **no migration**. Pure view
  state: it never touches DAG math, validation, or run order. `flows.save` already
  re-serializes the whole typed `Flow`, so it round-trips with zero route/host change.
- **TS** `ui/src/lib/flows/flows.types.ts` — `FlowNode.position?: {x, y}`.
- **Canvas** `ui/src/features/flows/flowGraph.ts` — `flowToNodes` uses
  `n.position ?? layout(i)` (stored wins, grid is the fallback); `nodesToFlowNodes`
  serializes `position` rounded to whole pixels (avoids churning the record with
  sub-pixel drag noise). The drag itself was already captured by React Flow's
  `applyNodeChanges` — it was only the save/load that lost it.

## Tests (green) + live proof

- `cargo test -p lb-flows` — 81/81 (added `position: None` to the one struct-literal
  test helper).
- `pnpm test flowGraph` — 12/12: stored position loads; missing position falls back to
  grid; a dragged fractional coord serializes rounded.
- Gateway test `FlowsCanvas.gateway.test.ts` — added a save→get position round-trip
  assertion (blocked by the branch-wide auth harness breakage, see below).
- **Live:** saved a flow with `{700,40}` + `{123,456}` via the real node, reloaded →
  coordinates returned exactly. Backend rebuilt + restarted first (model change → the
  stale-node trap, see [[flows-descriptor-served-not-hardcoded]]).

## Notes

- `pnpm test:gateway` fails "invalid or missing credential" on ALL flows tests on the
  `update-auth` branch (login-hardening broke `signInReal`) — pre-existing, not this
  change (see [[preexisting-failing-tests]]). Verified the real path via curl instead.
- `make dev` segfaulted twice from an "Address already in use" collision when launches
  stacked on a busy 8080 — a clean-port relaunch booted fine. Not related to this change.
