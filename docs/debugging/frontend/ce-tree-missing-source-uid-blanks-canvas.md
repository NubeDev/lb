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

## Root cause

The CE client crate `rubix-ce` (external git dep, pinned rev
`51ab97edf32d622f94d00401aee3ae2daf8859c8`) declares `EdgeDto.source_uid: u32` and
`target_uid: u32` as **required** (`src/types.rs:268`, no `#[serde(default)]`). The
real appliance's `GET /nodes?depth=..&withEdges=true` response contains an edge (at
byte ~7041) that **omits `source_uid`** — a dangling / half-formed edge. Serde fails
the **whole** `Tree` decode inside the crate's `get_tree`, so `control-engine.tree`
returns `CeError::Codec` → the extension replies "bad host response" → the editor has
no `{nodes,edges}` to render → blank canvas.

The strict typed hop bought the extension nothing: the wiresheet consumes the
`{nodes,edges}` **JSON verbatim** (`tools/tree.rs` already re-serializes the DTOs
straight through). Round-tripping edges through the crate's strict `EdgeDto` only
adds a crash the wire shape doesn't warrant.

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

## Regression test

`tools::raw_tree::tests` — a **rule-9** fixture in the real `/nodes` envelope shape
with a `source_uid`-less edge (`{"uid":9,"target_uid":1,...}`) asserts the edge passes
through instead of failing the decode, plus empty-halves defaulting and root/keyed uid
mapping. `cargo test -p control-engine --lib` → 23 passed (incl. the untouched fake
`tree_returns_seeded_graph_verbatim` path).

## Lesson

A strict typed transport hop that only re-serializes to JSON is pure downside: it can
only *reject* a wire shape the consumer would have tolerated. When the consumer speaks
raw JSON, fetch raw JSON — don't force it through a pinned external struct whose
required fields the real engine doesn't guarantee.
