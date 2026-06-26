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

## The membership graph (landed S4 — the second isolation layer)

S4 lands the first piece of the deferred membership graph, for **shared assets** (docs/skills):
a third gate *below* the workspace wall and *below* the capability check. `lb_host::get_doc` runs
ws → capability → **membership** (owner / member of a shared team / `sub`-grantee of a linked
channel); a member of team X cannot read a doc shared only to team Y, in the same workspace — the
second isolation layer this scope predicted. Membership is **SurrealDB relation records**
(`member` team→user, `share` doc→team, `link` doc→channel, `grant` skill→ws), re-resolved live on
every read, so a revoke is one delete — chosen over minting per-(member,resource) capabilities
(which would put membership in a token you must chase down). See `../files/files-scope.md`,
`../skills/skills-scope.md`, `../../sessions/files/shared-assets-session.md`.

## Open questions

- Membership model: ~~records + projection vs capabilities minted per (member, channel)~~
  **DECIDED (S4):** relation records, re-resolved live (above). Still open: move to SurrealDB
  `RELATE` graph edges when a second consumer (channel ACLs, tags) appears — the record names are
  chosen to make that a projection swap; and per-channel ACLs themselves (S4 reuses the channel
  `sub` capability for the doc→channel link path, not a separate channel-membership record yet).
- Per-channel visibility (`private`/`public` channels) and how it maps onto the `bus:chan/*`
  capability grammar.
- Workspace lifecycle: create/suspend/delete and what it does to the namespace + bus keys.
