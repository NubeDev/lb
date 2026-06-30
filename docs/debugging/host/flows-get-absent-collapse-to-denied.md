# `flows.get` on another workspace's (or deleted) flow returns `403`, not `404`

## Symptom

A gateway caller requesting another workspace's flow — or a tombstoned flow — gets `403 not
permitted` (opaque `Denied`), NOT the `404 NotFound` you'd expect from "the record isn't there". This
bit the Wave-3 flows-canvas tests first (they asserted `404` and failed with `403`).

## Root cause

`flows.get` runs the MCP authorize gate (`mcp:flows.get:call`) then a store-surface gate
(`store:flow:read`), then reads. The read of an absent/tombstoned flow returns `FlowsError::NotFound`
internally — but [`FlowsError::to_tool`](../../../../rust/crates/host/src/flows/error.rs) **collapses
`NotFound` to `ToolError::Denied`** (the MCP deny discipline: a caller must not learn whether a
resource exists vs. is forbidden — both paths a non-owner can reach return an opaque deny with no
existence signal). The gateway's `status()` then maps `Denied` → `403`.

This is the same existence-hiding the host flows tests pin
(`workspace_isolation_ws_b_cannot_see_ws_a_flow` asserts `ToolError::Denied` for an absent cross-ws
flow) — intentional, not a bug.

## Fix (expectation)

There is nothing to fix in the host. Callers/tests that assert "not found" must expect the **opaque
`403`/`Denied`** for an absent or cross-workspace flow — never `404`. The Wave-3 flows gateway test
(`flows_routes_test.rs`) and the canvas gateway test now assert `FORBIDDEN` / `rejects.toThrow()` for
these cases. The picker only ever lists reachable flows, so a user never sees this surface.

## Regression

`role/gateway/tests/flows_routes_test.rs::workspace_b_cannot_read_workspace_a_flow` and
`flows_crud_round_trip_over_the_gateway` (delete → get) assert the `403` (the existence-hiding
contract). If a future change makes `flows.get` return `404` for an absent flow, these fail —
re-add the existence leak only deliberately.
