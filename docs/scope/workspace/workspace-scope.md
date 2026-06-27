# Workspace scope - directory, session boundary, and lifecycle

Status: shipped, backfilled from code. Original slice: `scope/frontend/collaboration-scope.md`
plus the later admin CRUD work. Durable docs: `../../public/workspace/workspace.md`.

## Goal

Make workspaces explicit in the product surface without weakening the tenant wall:

- A signed session token names exactly one workspace.
- The UI can list and create workspace directory entries instead of hardcoding `acme`.
- Admins can archive, rename, and purge workspace entries through guarded lifecycle routes.
- Every workspace-owned data read still uses the token's workspace, not a request body.

## Model

A workspace id is the tenant id and the SurrealDB namespace. That namespace is the hard wall for
workspace data.

The workspace directory is different: it is node-level metadata held in the reserved namespace
`_lb_workspaces`, table `workspace`. The row shape is `WorkspaceRecord { ws, name, kind, status, ts }`.
It exists so a node can show "which workspaces are known here". It does not provision workspace data;
the data namespace appears on first write.

`WorkspaceStatus` is:

- `Active`: visible in the default list.
- `Archived`: hidden from the default list and retained for reversal.
- purged tombstone: `kind = "__purged__"`, final, not resurrected by create or rename.

## Host surface

Implemented in `rust/crates/host/src/workspaces/`:

| Verb | Code | Gate | Behavior |
|---|---|---|---|
| `workspace_create` | `create.rs` | `mcp:workspace.create:call` | Upserts a directory row unless a purge tombstone already exists. |
| `workspace_list` | `list.rs` | `mcp:workspace.list:call` | Lists active records from `_lb_workspaces`, ordered by logical `ts`. |
| `workspace_rename` | `rename.rs` | `mcp:workspace.delete:call` | Updates display name and marks the row active, so it is also unarchive. |
| `workspace_delete` | `delete.rs` | `mcp:workspace.delete:call` | Archives the row, leaving data retained. |
| `workspace_purge` | `delete.rs` | `mcp:workspace.purge:call` plus `confirm == ws` | Writes the purge tombstone. |

All gates authorize against `principal.ws()`, the session workspace. The directory is node-level, but
the authority to read or mutate it is still checked through the caller's current workspace grants.

## Gateway and UI

Gateway routes:

- `GET /workspaces` -> `workspace_list`
- `POST /workspaces` -> `workspace_create`
- `POST /admin/workspaces/{ws}/rename` -> `workspace_rename`
- `POST /admin/workspaces/{ws}/archive` -> `workspace_delete`
- `POST /admin/workspaces/{ws}/purge` -> `workspace_purge`

The browser client lives in `ui/src/lib/workspace/` and `ui/src/lib/admin/workspaces.api.ts`.
`WorkspaceSwitcher` lists the directory and creates entries. Switching workspaces is a re-login,
because the active workspace is a token claim, not a client-side selection. `WorkspacesAdmin` exposes
archive and purge, with the purge UI requiring type-the-name confirmation before the backend also
checks `confirm == ws`.

## Security invariants

- The session token carries the workspace. Routes authenticate first and derive `principal.ws()`.
- Request bodies do not choose the workspace for workspace-scoped data.
- Directory rows live in a reserved node namespace; tenant data remains in tenant namespaces.
- Archive is reversible and preserves data; purge writes a tombstone so stale sync or create calls
  cannot resurrect the id.
- Purge needs a distinct cap and a typed confirmation token.

## Tests

Covered by:

- `rust/crates/host/tests/collaboration_test.rs`: deny for list/create and workspace isolation for
  the collaboration host surface.
- `rust/role/gateway/tests/gateway_test.rs`: real signed sessions, forged/expired token rejection,
  and proof that the workspace comes from the token.
- `ui/src/features/admin/WorkspacesAdmin.gateway.test.tsx`: archive and purge leave the active
  directory list through real gateway routes.

## Open questions

- Full workspace provisioning remains separate. Creating a directory row makes the workspace
  listable; it does not seed users, teams, dashboards, docs, or policies.
- Real identity provider integration remains behind the same signed-token seam.
- Automatic namespace garbage collection after purge is a store follow-up.
