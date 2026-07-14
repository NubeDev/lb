# Schema validation ran before the cap gate — a denied caller got `400`, not an opaque `403`

- **Date:** 2026-07-14
- **Area:** auth-caps (the `/mcp/call` dispatch gate)
- **Status:** fixed
- **Found by:** triaging a red `viewer_reach_test` on clean `master` (`57684b1`)

## Symptom

`role/gateway/tests/viewer_reach_test.rs` asserts a `viewer` is denied every authoring verb. Six of
the seven rows passed; `dashboard.save` failed:

```
assertion `left == right` failed: a VIEWER must be denied the authoring verb `dashboard.save`
  left: 400
 right: 403
```

The obvious reading — "the test payload is stale, add the required `now` arg" — is **wrong and
dangerous**. Adding `now` does turn the row green, but only by routing around the bug: it makes the
call well-formed enough to reach the gate that was always going to deny it. The row would then pass
while no longer testing what it claims.

## Root cause

`host/src/tool_call.rs::dispatch_at_depth` ran the defense-in-depth JSON-Schema validator **before**
`authorize_tool`:

```rust
// validate `input` against the tool's declared schema …
if let Some(schema) = descriptor_schema(node, qualified_tool) {
    crate::tools::validate_args(Some(&schema), &input)?;   // ← BadInput => 400
}

if is_host_native(qualified_tool) {
    authorize_tool(principal, ws, gate_tool_for(qualified_tool))?;   // ← Denied => 403
```

`BadInput` maps to `400` and a deny to an opaque `403` (`role/gateway/src/routes/mcp.rs`). So for any
verb that **declares an `input_schema`**, an unauthorized caller was told *"your arguments are
malformed"* about a verb they have no right to call — a shape/existence oracle that contradicts the
contract the code states in two places:

- `tool_call.rs`: "a denied bridged caller is opaque and indistinguishable from a missing tool";
- `mcp/src/call/mod.rs`: "authorize runs before dispatch and — critically — a denied caller learns
  nothing about whether the tool exists."

Only the schema-declaring verbs leaked, which is why exactly one of the seven rows failed.
`dashboard.save` declares `required: [id, title, cells, now]` (added so a live agent could form the
call — see `debugging/agent/dashboard-save-cells-sent-as-json-string.md`), so it was the one row that
hit the validator first. The other six declare no schema and fell through to the gate correctly.

The extension tier was never affected: it dispatches via `lb_mcp::call_with_ctx`, which authorizes
first by design. This was a host-native-only divergence from the platform's own rule.

## Why the test caught it and then stopped catching it

This is the part worth remembering. The bug **disabled the assertion that would have caught it**: the
`dashboard.save` row stopped exercising the deny path the moment the verb gained a schema. A reviewer
who "fixed the stale payload" would have restored a green suite over a live leak, and the next person
to add a schema to a gated verb would silently re-open it.

## Fix

Move the validator to run **after** each tier's `authorize_tool`, extracted into
`validate_declared_args` and called immediately after the host-native gate. The stated intent of the
validator (a structurally bad request is a clean `BadInput`, never a panic deep in a handler) is
fully preserved for authorized callers — it just no longer answers questions for denied ones.

## Regression test

`viewer_reach_test::a_denied_caller_gets_an_opaque_403_even_when_the_args_are_malformed` — a viewer
calls `dashboard.save` twice over the real gateway: once with schema-violating args, once with valid
args. Both must be `403`, i.e. indistinguishable.

Verified to fail against the bug and pass with the fix:

| ordering | `…malformed_args` | `…viewer_cannot_reach…` |
|---|---|---|
| validate → gate (bug) | FAILED (400 ≠ 403) | FAILED (400 ≠ 403) |
| gate → validate (fix) | ok | ok |

Both rows go red against the bug, confirming the ordering — not the payload — was the cause.

## Lesson

At a security chokepoint, ordering *is* the contract. A validator that runs before the gate is not
"defense in depth", it is an oracle. When a deny test fails with a **non-403 error status**, the
first question is "did my request reach the gate at all?" — a `400` on a deny assertion means the
test has stopped testing the deny, not that the payload needs topping up.
