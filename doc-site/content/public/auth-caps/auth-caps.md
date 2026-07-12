# Auth-caps

TODO — public documentation for the auth-caps topic is written when its scopes ship.

This topic covers the capability grammar, tokens, the durable grant/role/team store, and
the three enforcement gates (workspace → capability → membership). See the scope docs under
`docs/scope/auth-caps/` for the current asks.

## MCP dispatch of authz admin verbs

TODO (fill on ship of `docs/scope/auth-caps/authz-verbs-mcp-dispatch-scope.md`): the
`grants.*` / `roles.* / teams.*` admin verbs are reachable both through their gateway REST
routes (the Access console) **and** through the generic `POST /mcp/call` host-callback
bridge, so a native (Tier-2) extension can mint/revoke scoped grants under the same admin
caps — closing the `entity-scoped-grants` derivation path for the native tier.
