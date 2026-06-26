# Tenancy scope

Status: scope. The workspace wall is the project's hardest invariant; this records how it is
enforced *structurally* on each surface, and what is still open (members/teams/channels graph).

> Read with: `../../README.md` §7 (workspace = tenant), §3.6 (isolation is gate 1),
> `../auth-caps/auth-caps-scope.md` (the two-gate check), `../bus/bus-scope.md`.

---

## Goal

The **workspace is the hard wall**: every key — store, bus, secret, mcp — is scoped by
workspace, and isolation is checked *before* any capability. A principal in workspace B must
never reach workspace A's data, even holding a matching capability. The wall is **structural**,
not a runtime `if` that could be forgotten.

## How the wall is structural on each surface (shipped S1–S2)

- **Auth (gate 1):** a token carries a single `ws` claim; `caps::check` refuses
  `principal.ws != request.ws` before any capability is read (`Denied::Workspace`). One
  chokepoint, every surface.
- **Store:** workspace = SurrealDB **namespace**. Every read/write/list selects the namespace
  from `ws` first, so a query for A physically cannot see B's records. (store isolation +
  list isolation tests.)
- **Bus:** every key is prefixed `ws/{id}/` by `lb_bus::ws_key` — callers never write the
  prefix, the host does. A peer for B cannot *name* A's keys; pub/sub and presence are all
  workspace-scoped. (messaging isolation test: a sub in B never receives a publish in A.)
- **Inbox:** items live in the workspace namespace via the store, so a channel `list` in B
  never returns A's items. (inbox isolation test.)
- **MCP:** a tool call's workspace is gate 1 of the same check; a cross-workspace call is denied
  before resolve (so it can't even tell the tool exists). (spine test.)

The mandatory **workspace-isolation** test category (testing §2.2) exists on every surface that
touches data — that is how the wall stays real as the surface grows.

## Non-goals (now)

- The **membership graph** — members, teams, and channel ACLs *within* a workspace. S2 treats a
  channel as authorized by a `bus:chan/{cid}:{pub|sub}` capability on the token; who-is-in-which-
  team and per-channel membership is a later layer on top of that, not a replacement for it.
- Cross-workspace sharing / federation — deliberately out of scope; the wall is the product.

## How it fits the core

- **Isolation first (§3.6):** gate 1 is workspace, gate 2 is capability — always that order, in
  one place (`caps::check`). No capability can widen the wall.
- **Symmetric nodes (§3.1):** the wall is the same on edge and cloud; nothing here branches on
  role.
- **Capability-first (§3.5):** within a workspace, a capability still gates the specific
  resource — the wall is necessary, not sufficient.

## Testing plan

- Shipped: workspace-isolation on store, list, bus, inbox, mcp (see the per-area test files).
- When the membership graph lands: a member of team X in workspace W cannot read a channel only
  team Y may see — *within* the same workspace (a second isolation layer below the wall).

## Open questions

- Membership model: members ↔ teams ↔ channels as SurrealDB records + a capability projection,
  or capabilities minted per (member, channel)? (Decide when the first multi-user app needs it.)
- Per-channel visibility (`private`/`public` channels) and how it maps onto the `bus:chan/*`
  capability grammar.
- Workspace lifecycle: create/suspend/delete and what it does to the namespace + bus keys.
