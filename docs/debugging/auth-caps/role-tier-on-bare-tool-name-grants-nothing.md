# A `[[tools]] role` tier on a bare tool name grants nothing

- Date: 2026-10-05
- Area: auth-caps (`host/src/authz/grant_role_tiers.rs`)
- Status: fixed

## Symptom

A native extension declared `role = "viewer"` on its read tools. After install, a member was still
refused every one of them (`denied`). `grants.list {subject: "role:member"}` and `role:viewer` were
empty, while `role:workspace-admin` held the same tools (through the `[ui].scope` grant). Found
driving ext-ros waste tracking on a rubix-ai scratch node.

## Root cause

`grant_role_tiers` built the cap from the `[[tools]]` name as written: `mcp:waste.summary:call`. A
`[[tools]]` name is bare by construction, and the host gates — and the install's `granted` set is
spelled — on the QUALIFIED name, `mcp:<ext>.waste.summary:call` (`ui_decl::qualify_tool`). The
"only grant what was granted" check therefore matched nothing, and every declared tier was skipped
without a log line. No extension declared a tier before this, so nothing had exercised it.

## Fix

Qualify through `ui_decl::qualify_tool` (now `pub(crate)`), the one place the bare → callable rule
lives.

## Regression test

`authz::grant_role_tiers::tests::a_bare_tool_name_is_granted_under_its_qualified_cap` — fails on the
old line (verified), passes with the fix. Plus `a_tier_never_widens_past_the_granted_set`.
