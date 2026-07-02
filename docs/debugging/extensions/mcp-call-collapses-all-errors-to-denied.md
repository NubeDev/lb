# A sidecar's `store.query` callback surfaces as `Denied`, hiding a SurrealQL parse error

- **Area:** extensions (native-sidecar callback) / gateway
- **Date:** 2026-07-02
- **Status:** resolved
- **Slice:** control-engine S4 ([session](../../sessions/control-engine/ce-v1-s4-session.md))

## Symptom

Building the `ce_appliance` registry (S4), `appliance.add` (via `store.write`) succeeded but
`appliance.list` (via `store.query`) failed with `HostError::Denied` — an opaque capability denial —
even though the sidecar's token clearly held `mcp:store.query:call`. The deny made it look like a
capability/grant bug, but the grant was correct.

## Root cause

Two layers collapse distinct failures onto one signal:

1. The gateway bridge `POST /mcp/call` (`role/gateway/src/routes/mcp.rs`) maps **every** `call_tool`
   error to `StatusCode::FORBIDDEN` — a deliberate no-existence-signal contract for capability denials,
   but it also swallows a `BadInput`.
2. `lb-sidecar-client` maps any `403` to `CallError::Denied` (`crates/sidecar-client/src/client.rs`).

So a `store.query` **parse rejection** (`StoreQueryError::Parse → ToolError::BadInput`) reached the
sidecar as `Denied`. The actual fault was the SurrealQL: `SELECT data FROM ce_appliance ORDER BY data.id`
fails to parse — "Missing order idiom `data.id` in statement selection" (the same class as
[store/order-by-needs-selected-idiom.md](../store/order-by-needs-selected-idiom.md); `store.query`'s
parse-allowlist runs it verbatim). The `WHERE data.id = $id` form parses fine, which is why `get`/`add`
worked and only `list` (the ORDER BY) failed.

## Fix

Drop the SQL `ORDER BY` and sort host-side in Rust after unwrapping the rows
(`extensions/control-engine/src/appliance/store.rs::list`). The parse gate keeps `store.query` to a
single projected-column SELECT, so ordering by a nested field is not expressible in SQL here anyway.

## How it was diagnosed (the reusable lesson)

Because `/mcp/call` hides the real error, drive the underlying host verb **directly** to see it: a
throwaway `#[tokio::test]` called `lb_host::store_query_run(&store, &p, ws, sql, vars)` with the exact
SQL and printed the `Err` — which showed `Parse(...)`, not a denial. When a native sidecar's callback
returns `Denied` but the grant looks right, suspect a `BadInput` collapsed to `403`; test the host verb
off the bridge.

## Regression test

`extensions/control-engine/tests/appliance_registry_test.rs::add_list_resolve_remove_round_trip` — `list`
returns the seeded record (fails before the fix, passes after). The generic
`crates/host/tests/store_mutate_test.rs` covers the mutation verbs directly (off the bridge), so a future
regression in the store path surfaces as the true error, not a masked `Denied`.
