# Session — `grants.*`/`roles.*`/`teams.*` over the MCP dispatcher (implementation)

Date: 2026-07-12. Branch: `authz-verbs-mcp-dispatch`. Implements
`docs/scope/auth-caps/authz-verbs-mcp-dispatch-scope.md`. All green.

## What shipped

The host MCP dispatcher now routes `grants.*` / `roles.*` / `teams.*` to `call_authz_tool`,
so the WRITE half of the scoped-grant surface is reachable over the host callback — symmetric
with the READ half (`authz.check_scoped` / `authz.scope_filter`) that was already live under the
`authz.` prefix. This unblocks a native (Tier-2) extension minting a scoped grant, e.g. cc-app's
guardianship-edge → row-level reach derivation.

Four touch points, all in `rust/crates/host/src/` (as the corrected scope's Intent/approach
§1–4 predicted — NOT the "one else if" the cc-app debugging record sketched):

1. `tool_call.rs` `HOST_NATIVE_PREFIXES` += `grants.` / `roles.` / `teams.`. This is the gate
   (`is_host_native`) that decides host-native vs extension routing; an arm without the prefix
   entry is dead code.
2. `tool_call.rs` dispatch arm: the existing `authz.` arm's condition now also matches the three
   prefixes, delegating to `call_authz_tool`.
3. `tool_call.rs` `gate_tool_for` — **the load-bearing part.** The outer MCP gate runs
   `mcp:<tool>:call` by the tool's own name; four verbs are inner-gated + role-bundled under a
   *different* cap, so they need aliases:
   - `grants.revoke` → `grants.assign` (assign/revoke share the cap — `authz/grants.rs:37,55`)
   - `grants.list_scoped` → `grants.list` (`authz/grants.rs:81`)
   - `teams.create` → `teams.manage` (`authz/teams.rs:24`; no `mcp:teams.create:call` exists)
   - `roles.delete` → `roles.manage` (`authz/roles.rs:61`)

   Verified each alias points at the exact cap the verb's inner `authorize_tool` checks. Without
   them, those four would deny even for a workspace-admin. (`roles.define` gates on its own name
   — no alias, correct.)
4. `system/catalog.rs`: nine descriptor rows under `group: "authz"`. The
   `host_catalog_covers_dispatch_prefixes` test hard-fails on any dispatched prefix with no
   catalog row, so these are mandatory, not optional.

## Tests (real bridge, real Node, real store — no mocks, testing-scope §0)

New file `rust/crates/host/tests/authz_mcp_dispatch_test.rs`, 5 tests, all green:

- `admin_reaches_all_nine_authz_verbs_over_the_bridge` — **the load-bearing test**. Admin token
  holds the admin caps by canonical name (`grants.assign`, `grants.list`, `teams.manage`,
  `roles.manage`, `roles.define`) but NOT `grants.revoke` / `grants.list_scoped` /
  `teams.create` / `roles.delete` by name. All nine dispatch. Without the §3 aliases exactly
  those four deny — which an assign-only test would miss.
- `assign_then_read_back_scoped_over_the_bridge` — read/write symmetry: assign a scoped grant,
  read it back via `grants.list_scoped`, same transport.
- `denies_each_verb_without_its_gate_cap_over_the_bridge` — mandatory capability-deny, per verb,
  opaque `Denied` (incl. the aliased verbs); `grants.list_scoped` allowed via its `grants.list`
  alias.
- `anti_widen_fires_over_the_bridge` — admin with `grants.assign` but not the target cap gets
  `BadInput` (the handler guard runs regardless of transport).
- `ws_b_admin_cannot_touch_ws_a_authz_over_the_bridge` — mandatory workspace-isolation: a token
  scoped to ws-A driving a call labelled ws-B is denied at gate 1 (`caps::check`
  `principal.ws() != req.ws`).

Regression: existing `authz_test` (7) + `authz_scoped_test` (9) + catalog coverage — all green.
Total 21 authz tests pass. `cargo fmt` clean, `cargo build -p lb-host` clean.

## Notes / accepted trade-offs

- MCP-dispatched calls auto-capture into the undo journal at depth 0 (`call_tool_at_depth`); the
  REST admin routes don't. A grant write over the callback gains an undo entry its REST twin
  lacks — platform-standard for every bridged verb, not special to authz. "Zero behavior change"
  is exact only for existing callers; the new transport carries the bridge's standard semantics
  (undo capture, telemetry dispatch record, schema pre-validation).
- No new verb, cap, grammar, WIT, table, or migration. Additive routing only.

## Follow-up for cc-app

The patch cc-app validated in `grants-verbs-not-on-mcp-callback-surface.md` was directionally
right but incomplete against `node-v0.3.1`'s dispatcher (it omitted the `HOST_NATIVE_PREFIXES`
entry and the four gate aliases — with those missing, revoke/create/delete/list_scoped deny even
for admins). This branch lands the complete fix; cc-app should consume the tag it ships under.
