# Auth-caps

TODO — public documentation for the auth-caps topic is written when its scopes ship.

This topic covers the capability grammar, tokens, the durable grant/role/team store, and
the three enforcement gates (workspace → capability → membership). See the scope docs under
`docs/scope/auth-caps/` for the current asks.

## MCP dispatch of authz admin verbs

The `grants.*` / `roles.*` / `teams.*` admin verbs are reachable **two ways**, both under the
same admin capabilities:

1. their gateway REST routes (`POST /admin/grants`, …), which the Access console uses; and
2. the generic `POST /mcp/call` host-callback bridge — the one MCP contract every host-native
   verb rides.

So a **native (Tier-2) extension** can mint/revoke a scoped grant over the host callback, not
just *read* the scoped-grant surface (`authz.check_scoped` / `authz.scope_filter`, always
callback-reachable). This closes the `entity-scoped-grants` derivation path for the native tier
— e.g. an extension links a domain edge (a guardianship, a site assignment) and derives the
edge's row-level reach with `grants.assign(subject, cap, scope)` in the same transaction, then
removes it with `grants.revoke` when the edge is unlinked.

**Capabilities are unchanged.** A callback caller is gated by the *same* cap a console admin
needs — `mcp:grants.assign:call` for assign/revoke, `mcp:grants.list:call` for the list verbs,
`mcp:teams.manage:call` for team create, `mcp:roles.manage:call` for role delete,
`mcp:roles.define:call` for role define. The callback is a second transport to the same gate,
never a privilege escalation: workspace-isolation is checked first, then the capability, then
the handler's own anti-widen guard (you cannot grant a cap you do not hold). An ungranted caller
gets an opaque `Denied` with no existence signal.

The verbs (nine): `grants.assign`, `grants.revoke`, `grants.list`, `grants.list_scoped`,
`roles.define`, `roles.list`, `roles.delete`, `teams.create`, `teams.list`.
