# CE canvas: no live values — `control-engine.watch` subscribed an EMPTY COV scope

- **Date:** 2026-07-03
- **Area:** frontend / control-engine sidecar (`watch` verb)
- **Status:** resolved
- **Branch:** `ce-node-wiring-v2`

## Symptom

With the tree decode fixed ([ce-tree-missing-source-uid-blanks-canvas.md](ce-tree-missing-source-uid-blanks-canvas.md))
and the shell bridge `watch` wired ([ce-canvas-disconnected-no-live-values-bridge-missing-watch.md](ce-canvas-disconnected-no-live-values-bridge-missing-watch.md)),
the Control Engine canvas rendered the graph + wires but still showed **no live
property values** and the badge read **disconnected**. Driving the live path
(`control-engine.watch {appliance:"aaaa"}` → `GET /series/{s}/stream`) opened the
series but streamed **zero bytes**.

## Root cause

`control-engine.watch` with no explicit `scope` built an **empty `CovScope`** (no
component/property UIDs). The crate's `subscribe` then sent `{"type":"subscribe"}`
with no `components`, and **the ce-studio engine only pushes COV frames for
explicitly-subscribed components** — so an empty subscribe carries nothing.

Proven with a raw WS client to `:7979/` (bypassing all LB/crate code):
- subscribe with **no** components → schema + presence frames, **0 value frames** in 3.5s.
- subscribe with `"components":[100008,…,100013]` (the real children) → **36 value frames** in 4s.

So the pump armed fine, the SSE authenticated fine — there was simply **nothing to
stream** because we subscribed to nothing. The UI's default watch is "watch the whole
appliance" (`{ appliance }`, no scope), which is exactly the empty-scope case.

## Fix

Extension-side (self-contained). In the watch verb
([rust/extensions/control-engine/src/watch/verb.rs](../../../rust/extensions/control-engine/src/watch/verb.rs)),
before arming, `expand_scope` turns an EMPTY scope into the appliance's full
component-UID set:

- if the caller gave `scope.components` (or `scope.properties`) → honour it verbatim
  (an explicit scope is a deliberate narrowing; never widen it);
- otherwise fetch the tolerant raw tree (`tools::raw_tree`, the same camelCase-safe
  pass-through the canvas uses) and collect **every** component `uid`
  ([watch/scope_uids.rs](../../../rust/extensions/control-engine/src/watch/scope_uids.rs),
  a recursive walk that skips the synthetic root `uid 0`), then inject them as
  `scope.components`;
- a tree-fetch failure (engine unreachable) is **non-fatal**: fall back to the given
  scope so the watch still arms (a gap, not a failed call).

The expanded scope keys a **different** series (`series::args_hash` folds the scope),
which is the whole point: the empty-scope series carried nothing; the populated-scope
series is what the pump enumerates and streams. The verb resolves both the series and
the scope in one call, so the arm and the UI's read stay consistent.

## Verification (live)

```
control-engine.watch {appliance:"aaaa"}  → series ce-cov:aaaa:3a3793cf5fd8ee79
                                           (was ce-cov:aaaa:5ff290f56d60cce8 — empty scope)
GET /series/<s>/stream                    BEFORE: 0 bytes · AFTER: 3164 bytes, 9 `event: sample`
```

AFTER frames carry real COV payloads, e.g.
`{"kind":"cov","values":[{"uid":1000072,"v":11867},{"uid":1000077,"v":6034.165},…]}`.
Rebuilt `cargo build -p control-engine` + republished (`make kill && make dev` — the
node does NOT hot-reload Rust) so the live sidecar picked up the fix.

## Regression test

[rust/extensions/control-engine/tests/watch_scope_expand_test.rs](../../../rust/extensions/control-engine/tests/watch_scope_expand_test.rs)
drives the REAL `expand_scope` → `raw_tree` → `scope_uids::collect` path over a REAL
HTTP round-trip: a live `axum` server serves a **captured-real** `/api/v0/nodes`
envelope (the exact live-engine shape). Asserts (1) an empty scope expands to every
component UID and keys a different series than the empty one, (2) an explicit scope is
never widened, (3) an unreachable engine falls back to the given scope. Plus
`scope_uids.rs` unit tests over the captured tree shape (nested/array/object children,
root excluded). No fakes of node behavior — only a genuine external HTTP endpoint stood
up locally (rule 9).

## Lesson

An "empty means everything" convention is a trap when the downstream (the engine) reads
empty as "nothing." The default watch (whole appliance) MUST be materialized into the
concrete UID set the engine understands, at the arming boundary — leaving the scope
literally empty makes the pump, the series, and the SSE all "work" while carrying zero
data.

## Adjacent (noted, not the values bug)

The crate's WS `SchemaMsg`/`SchemaProperty` are snake_case (`session_id`) but the engine
sends **camelCase** (`sessionId`) — `#[serde(default)]` swallows it, so `session_id`
silently decodes to `""`, breaking WS **resume/gap-detection** (same camelCase mismatch
the REST `EdgeDto` had). Not the cause of "no values" (that's the empty scope). Left as a
follow-up: it lives in the pinned `ce-client-rust` git dep (`src/ws/control.rs`), not in
this repo, so it needs a crate rev bump with `#[serde(rename_all = "camelCase")]`, not an
in-repo edit.
