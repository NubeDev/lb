# MCP

TODO — filled when the MCP layer's public docs are written.

The MCP tool layer is how capabilities are exposed and called: a qualified tool name
(`<ext>.<tool>`), a capability check (`mcp:<ext>.<tool>:call`), and a dispatch that may be local
or routed to another node.

Scopes behind this page:

- `docs/scope/mcp/mcp-scope.md` — the tool layer and the caller contract.
- `docs/scope/mcp/routed-node-dispatch-scope.md` — addressing a **named node**, so a fleet running
  the same extension is reachable (and an ambiguous call is an error rather than a coin flip).
- `docs/scope/mcp/ems-provisioning-verb-shapes-scope.md` — the confirmed wire shapes for the
  host-native provisioning verbs.
