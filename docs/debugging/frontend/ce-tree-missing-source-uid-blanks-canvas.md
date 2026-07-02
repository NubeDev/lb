# control-engine canvas blank — `control-engine.tree` codec crash on `missing field source_uid`

- **Date:** 2026-07-03
- **Area:** frontend / control-engine extension
- **Status:** resolved
- **Branch:** `ce-node-wiring-v2`

## Symptom

Opening the Control Engine page rendered a **blank canvas** under a red error toast:

```
extension error: child returned an error: bad host response: control-engine codec
error: /nodes?depth=1&withEdges=true: decoding response: missing field `source_uid`
at line 1 column 7041
```

The editor received no graph, so nothing drew.

## Root cause (confirmed against the live ce-studio engine)

The error message ("missing field `source_uid`") pointed at a single dangling edge,
but capturing the REAL `/nodes` payload from the running engine (`curl
127.0.0.1:7979/api/v0/nodes?depth=-1&withEdges=true`) showed the real story: the
engine emits **camelCase** keys everywhere —

```json
{ "uid": 1000000, "sourceUid": 100010, "targetUid": 100011,
  "sourcePropertyUid": 1000115, "sourceProperty": "out", ... }
```

The CE client crate `rubix-ce` (external git dep, pinned rev
`51ab97edf32d622f94d00401aee3ae2daf8859c8`) declares `EdgeDto` with **snake_case**
required fields `source_uid`/`target_uid` and **no** `#[serde(rename_all)]`
(`src/types.rs:268`). So it isn't ONE edge that's malformed — **every** edge fails
(`sourceUid` ≠ `source_uid`), and serde fails the whole `Tree` decode inside the
crate's `get_tree` → `control-engine.tree` returns `CeError::Codec` → the extension
replies "bad host response" → the editor has no `{nodes,edges}` → blank canvas.

The strict typed hop was **pure downside** and doubly wrong: the wiresheet's own
`engine-types.ts` (`Edge.sourceUid`, `Component.childrenCount`, …) and its
`rfbuild.ts` read the RAW **camelCase** engine shape directly (ce-wiresheet connects
straight to the engine outside LB). Round-tripping through the crate's snake_case
`EdgeDto` would (a) crash on the naming mismatch, and even if it hadn't, (b)
re-serialize to snake_case the wiresheet can't read. The crate was the ONLY consumer
in the whole chain that wanted snake_case.

## Fix

Bypass the typed decode for the REAL-appliance read with a **tolerant raw fetch**
(`rust/extensions/control-engine/src/tools/raw_tree.rs`): the extension does its own
`GET http://{host}:{port}/api/v0/nodes?depth=..&withEdges=true`, unwraps the
`{ "data": { nodes, edges } }` envelope as untyped `serde_json::Value`, and passes it
straight through. A `source_uid`-less edge survives as-is — the wiresheet decides how
to render a dangling edge, not the transport. Only the strict edge-shape decode is
relaxed; transport errors and non-2xx still surface.

`serve.rs` routes `control-engine.tree` to `raw_tree::run(&base, input)` for the real
path (`!is_fake()`); the sanctioned `ce_fake` stub keeps the typed `get_tree`
(`tools::dispatch`) since it drives the trait, not real HTTP. Added `reqwest` as a
direct dep of the extension (same `rustls-tls` pin the workspace uses; Cargo unifies
it with the `reqwest` `rubix-ce` already pulls).

This is the extension-side fix (handover option #2) rather than the upstream
`Option<u32>` change — the crate is an external git dep we can't edit in-repo, and the
wiresheet never needed the typed edge anyway. If the upstream crate later makes the
fields optional, this path still works (it never decoded them).

Note this ALSO explains issue #3 (nodes render without input/output slots; values
don't stream): it was entirely downstream of this crash — the decode failed, so the
editor never received the component graph, so `buildRfNodes`/`buildRfEdges` had no
children/ports/edges to draw. With the tolerant fetch delivering the raw camelCase
graph, the slots and wires resolve (the wiresheet reads `properties` for ports and
`sourceUid`/`targetUid` + `sourceProperty`/`targetProperty` for edges).

## Regression test

Two layers, both rule-9 (real captured shape / real live engine):
- `tools::raw_tree::tests` (Rust) — a fixture in the real `/nodes` envelope shape with
  a `source_uid`-less edge asserts pass-through, plus empty-halves defaulting and
  root/keyed uid mapping. `cargo test -p control-engine --lib` → 23 passed (incl. the
  untouched fake `tree_returns_seeded_graph_verbatim` path).
- `bridge-transport.live.test.ts` (ext UI, env-gated `CE_ENGINE_URL` +
  `CONTROL_ENGINE_BIN`) — spawns the REAL sidecar against the live ce-studio and
  asserts `GET /nodes?withEdges=true` returns real **camelCase** edges
  (`typeof e.sourceUid === "number"`) through the full bridge, i.e. the exact crash no
  longer happens end-to-end. 4/4 passed against the running engine.

## Lesson

A strict typed transport hop that only re-serializes to JSON is pure downside: it can
only *reject* a wire shape the consumer would have tolerated. When the consumer speaks
raw JSON, fetch raw JSON — don't force it through a pinned external struct whose
required fields the real engine doesn't guarantee.
