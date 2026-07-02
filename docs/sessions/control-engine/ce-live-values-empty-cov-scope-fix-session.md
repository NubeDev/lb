# Session — CE live values: expand the empty COV subscribe scope (final fix)

- **Date:** 2026-07-03
- **Branch:** `ce-node-wiring-v2` (stayed on it)
- **Predecessor:** [ce-handover-live-values-empty-cov-scope.md](ce-handover-live-values-empty-cov-scope.md)
  (root-caused the bug; this session implemented + verified the fix).

## The ask

One remaining bug from the handover: the CE canvas shows no live values because
`control-engine.watch` subscribes with an **empty `CovScope`**, and the ce-studio engine
only pushes COV frames for explicitly-subscribed components. Fix it extension-side:
before arming, fetch the tree and populate `CovScope.components` with every component UID
for the appliance.

## What I changed

1. **New `watch/scope_uids.rs`** — one responsibility: recursively walk the tolerant raw
   tree's `nodes` (each node may carry nested `children` as an array or object map) and
   collect every component `uid` into a sorted+deduped list. The synthetic root (`uid 0`)
   is excluded — it bears no COV. Sorted so it feeds `series::args_hash` deterministically.
2. **`watch/verb.rs` — `expand_scope`** — before `target(...)`/`arm(...)`, expand an empty
   scope:
   - caller gave `scope.components` or `scope.properties` → honour verbatim (never widen an
     explicit narrowing);
   - else fetch `tools::raw_tree(base, input)` (the same camelCase-safe pass-through the
     canvas uses; no `node`/`depth` args → whole tree at depth `-1`), `scope_uids::collect`
     it, and inject the UIDs as `scope.components`;
   - tree-fetch failure (engine unreachable) or an empty tree → return input unchanged
     (**non-fatal**: the watch still arms — a gap, not a failed call).
   The verb resolves the (now populated) series AND scope in one call, so the arm and the
   UI's read stay consistent. `expand_scope` is `pub` so the integration test drives it.
3. **`watch/mod.rs`** — declared `pub mod scope_uids`.

## Verification (live, rule 9)

Rebuilt `cargo build -p control-engine`, then **republished** (`make kill && make dev
CE_BASE=127.0.0.1:7979 CE_APPLIANCE=aaaa …`) so the live sidecar picked up the fix (the
node does NOT hot-reload Rust). Appliance is **`aaaa`** (not `local`).

| | series | `/series/<s>/stream` |
| --- | --- | --- |
| BEFORE | `ce-cov:aaaa:5ff290f56d60cce8` (empty scope) | 0 bytes (hangs) |
| AFTER | `ce-cov:aaaa:3a3793cf5fd8ee79` (expanded scope) | 3164 bytes, **9 `event: sample`** |

AFTER frames carry real COV, e.g.
`{"kind":"cov","values":[{"uid":1000072,"v":11867},{"uid":1000077,"v":6034.165},…]}`.
The browser's `series.stream.ts` consumes exactly this byte stream through the shell
bridge `watch` → `openSeriesStream` path (already wired last session), so the canvas
populates.

## Tests (all green)

- `cargo test -p control-engine --features ce-fake` → 26 unit + 3 new integration + 6
  appliance + lifecycle, **0 failed**. `cargo fmt` clean.
- **New regression** `tests/watch_scope_expand_test.rs` — drives the REAL
  `expand_scope`→`raw_tree`→`scope_uids::collect` path over a REAL HTTP round-trip: a live
  `axum` server serves a **captured-real** `/api/v0/nodes` envelope (the live-engine
  shape). Asserts: empty scope → every UID + different series; explicit scope → unchanged;
  unreachable engine → falls back. No fake of node behavior (only an external HTTP endpoint
  stood up locally — what rule 9 permits).
- `scope_uids.rs` unit tests over the captured tree shape (nested array/object children,
  root excluded, empty/shapeless tolerance).

## Docs

- Debugging: [../../debugging/frontend/ce-canvas-empty-cov-scope-no-live-values.md](../../debugging/frontend/ce-canvas-empty-cov-scope-no-live-values.md)
  (opened + closed) and a README history row.
- This closes the handover's one remaining item.

## Carried-forward note (not this bug)

The pinned `ce-client-rust` WS `SchemaMsg`/`SchemaProperty` are snake_case (`session_id`)
while the engine sends camelCase (`sessionId`); `#[serde(default)]` swallows it → `""` →
broken WS **resume/gap-detection**. It's in the uneditable git dep (`src/ws/control.rs`),
so it needs a crate rev bump with `#[serde(rename_all = "camelCase")]`, not an in-repo
edit. Not touched this session (would mean bumping the dep); logged in the debugging entry
so it isn't lost.
