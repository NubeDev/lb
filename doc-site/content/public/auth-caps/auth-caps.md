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

## Subject-scoped `bus.watch` grants (per-entity motion isolation)

The generic bus subscribe (`bus.watch`, backing `GET /bus/{subject}/stream` and the multiplexed
`bus:` event subject) is gated coarsely by the workspace-wide `mcp:bus.watch:call`. That cap alone
lets any holder in a workspace watch **any** `ext/*` subject — fine for a shared feed, wrong for a
**per-entity** one (a live feed keyed by a child, a device, an order).

A **subject-scoped grant** narrows it, converging on the channel service's `bus:chan/*:sub` idiom:
the cap is `bus:<subject>:watch` (surface `bus`, the subject as the resource, action `watch`, with
the usual `*`/`**` wildcards — `bus:care.feed.*:watch` matches `care.feed.leo` but not
`other.feed.x`).

- **Coarse gate unchanged.** `mcp:bus.watch:call` is still required (workspace-first, then the cap).
- **Present ⇒ required, absent ⇒ open.** If the caller holds **no** `bus:*:watch` grant, behaviour
  is exactly as before (every subject reachable — fully backward-compatible). If the caller holds
  **any** `bus:*:watch` grant, they are under subject enforcement: the watched subject needs a
  matching grant, else an opaque `Denied`. The scoped read is **live** (a store read, not the
  token), so a grant assigned after login authorizes on the next subscribe.
- **Revoke terminates the stream.** An open stream re-checks its grant on a bounded tick (a few
  seconds); revoking the matching grant **closes** the stream. The requirement is anchored to the
  grant, so revoking a caller's *last* grant denies the subject — it never silently re-opens under
  back-compat mode.

An extension mints/revokes the grant through the same generic `grants.assign` / `grants.revoke`
verbs (above) — the cap string is opaque data to the core. Typical flow: on linking an entity to a
principal, `grants.assign(user, "bus:<entity-subject>:watch")`; on unlinking, `grants.revoke(...)`,
which closes any open stream within a tick.
