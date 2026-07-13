# Session — review of `authz-verbs-mcp-dispatch-scope.md` (cc-app's ask)

Date: 2026-07-12. Docs-only session: verified the cc-app-filed scope against the dispatcher
code and corrected its implementation sketch. No code changed, no tests run (nothing to run —
the scope is still "the ask").

## What was checked

The scope's central claim — `call_authz_tool` implements `grants.*`/`roles.*`/`teams.*` but the
dispatcher never routes them — is **true** (`rust/crates/host/src/authz/tool.rs` matches the
bare names; `tool_call.rs` routes only the `authz.` prefix). The ask is legitimate and the
approach (route the real names, no new verbs/caps/WIT) is right.

## What was wrong and got fixed in the scope doc

1. **The "one `else if` arm" sketch was dead code.** The host-native dispatch block sits behind
   `is_host_native()` / `HOST_NATIVE_PREFIXES` (`tool_call.rs:54`); an arm for a prefix not in
   that const is never reached. Real change = prefix-const entry + arm.
2. **Four of nine verbs would deny even for admins.** The outer gate runs
   `mcp:<tool>:call` by name (`gate_tool_for` default), but `grants.revoke` /
   `grants.list_scoped` / `teams.create` / `roles.delete` are inner-gated and role-bundled
   under *different* caps (`grants.assign` / `grants.list` / `teams.manage` / `roles.manage` —
   see `authz/{grants,teams,roles}.rs` and `builtin_roles.rs::ADMIN_ONLY_CAPS`). They need
   `gate_tool_for` aliases, same pattern as `outbox.enqueue_held`. Without them the scope's own
   example flow (unlink → `grants.revoke`) fails with an unfixable deny.
3. **Catalog mirror is enforced by test.** `system/catalog.rs::host_catalog_covers_dispatch_prefixes`
   hard-fails on any `HOST_NATIVE_PREFIXES` entry with no catalog row — nine descriptor rows
   are part of the change, not optional.
4. Noted the standard bridge semantics the new transport carries (undo auto-capture at depth 0,
   telemetry dispatch record) as an accepted asymmetry vs the REST path.

Testing plan gained two cases: the aliased-verb gate-agreement test (an admin token can call
all nine — catches exactly the four-deny failure) and catalog/palette coverage.

## Follow-up

The implementing session should treat the scope's Intent / approach §1–4 as the checklist and
tell cc-app that the patch it validated (`grants-verbs-not-on-mcp-callback-surface.md`) is
directionally right but incomplete against `node-v0.3.1`'s dispatcher.
