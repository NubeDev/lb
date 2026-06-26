# Tenancy (as built)

The **workspace is the hard wall** — every key is scoped by workspace, and isolation is checked
*before* any capability. The wall is **structural**, not a runtime `if`. Promoted from
`scope/tenancy/` after S2 (the wall now holds across the messaging surfaces too).

## The wall, surface by surface

| Surface | How isolation is structural |
|---|---|
| **Auth (gate 1)** | token has one `ws` claim; `caps::check` refuses `principal.ws != request.ws` before any capability — `Denied::Workspace`. |
| **Store** | workspace = SurrealDB **namespace**; every read/write/list selects it from `ws` first. |
| **Bus** | every key prefixed `ws/{id}/` by `ws_key`; a peer for B can't *name* A's keys (pub/sub + presence). |
| **Inbox** | items live in the workspace namespace; a channel `list` in B never returns A's items. |
| **MCP** | the call's workspace is gate 1; a cross-workspace call is denied before resolve (can't even tell the tool exists). |

The mandatory **workspace-isolation** test category exists on every surface that touches data —
that is how the wall stays real as the product grows.

## Order matters

Gate 1 is **workspace**, gate 2 is **capability** — always that order, in one place
(`caps::check`). No capability can widen the wall; within a workspace, a capability still gates
the specific resource.

## Not yet built

The **membership graph** within a workspace (members ↔ teams ↔ channel ACLs) — S2 authorizes a
channel by a `bus:chan/{cid}:{pub|sub}` capability on the token; per-team/per-channel membership
is a later layer *below* the wall. Also: channel visibility (private/public) and workspace
lifecycle. See `scope/tenancy/` open questions.
