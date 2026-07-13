# Session: wire `channel.create` into MCP dispatch

**Issue:** [NubeDev/lb#52](https://github.com/NubeDev/lb/issues/52) — `channel.create` was
advertised as a callable capability (`mcp:channel.create:call`) and its prefix (`channel.`) is
host-native, but the dispatcher `call_channel_tool` had no `"channel.create"` arm, so the call
fell through to `_ => Err(ToolError::NotFound)`. Channels were therefore unprovisionable over the
callback: an extension could `channel.post`/`history`/`list` but could not explicitly `create` a
channel so it is listable before the first post.

## Root cause

The in-process `channel_create` helper (`channel_registry/create.rs`, re-exported at
`lib.rs:156`) already existed and is used by the WS / `POST /channels` path. Only the MCP dispatch
arm was missing. The outer `mcp:channel.create:call` gate passed; dispatch entered the `channel.`
arm; the match had no `create` arm ⇒ `NotFound`, so the inner `bus:chan/{cid}:pub` gate was never
reached.

## Fix (additive — no new capability, no ABI/SDK change)

1. `channel/tool.rs` — added a `"channel.create"` arm that reads `cid` (+ optional `ts`) and calls
   the existing `crate::channel_create(&node.store, principal, ws, cid, ts)`, mapping errors via the
   existing `chan_err`. That helper runs `authorize_channel(..., Action::Pub)`, so a caller holding
   `bus:chan/{cid}:pub` is authorized and one without is **Denied** (opaque), not NotFound.
   Create reuses the channel `pub` gate per the collaboration-scope decision that "creating a
   channel is exactly 'may I post here'" — no new cap.
2. `channel/tool.rs` — added `create_descriptor()` for catalog parity with `post_descriptor()`
   (`cid` required, `ts` optional); exported from `channel/mod.rs`; registered in
   `tools/descriptor.rs::host_descriptors()`.
3. `system/catalog.rs` — added the `channel.create` `HostTool` catalog row.

## Tests (`crates/host/tests/channel_mcp_test.rs`, real booted Node — no mocks, CLAUDE §9)

- `create_makes_channel_listable_before_any_post_and_is_idempotent` — create → `channel.list`
  surfaces it before any post; create-then-create settles (idempotent upsert).
- `create_denied_without_pub_cap_is_opaque` — caller with the MCP door but not `bus:chan/*:pub`
  is **Denied** (not NotFound) and no channel is registered (capability-deny + no-write).
- `full_caps()` extended with `mcp:channel.create:call`.

`cargo test -p lb-host --test channel_mcp_test` → **7 passed**.

## Pre-existing (not this change)

`agent_persona_catalog_test` has 6 failures on clean master (confirmed via `git stash` — identical
before and after). Unrelated; see the `preexisting-failing-tests` note.

## Downstream

Unblocks `NubeDev/cc-app` `care.channel.reconcile` (milestone 09/10 messaging): the `care` native
extension can now provision child/room channels on-demand over the host callback. `grants.assign`
membership derivation already worked.
