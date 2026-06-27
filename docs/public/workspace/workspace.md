# Workspace

The shipped workspace surface has two related responsibilities:

1. The session boundary: a signed token names one workspace, and every workspace-owned data read or
   write derives the workspace from that token.
2. The node directory: a small reserved-namespace list of workspaces that the UI can show and admins
   can manage.

## Session boundary

`POST /login` mints an `lb_auth` token with `sub`, `ws`, role, caps, `iat`, and `exp`. Every gateway
route authenticates first and derives a `Principal`; the route then calls host verbs with
`principal.ws()`. The browser can pass convenience `ws` arguments in some API calls, but the gateway
path does not trust them for isolation.

This is the isolation rule: workspace data is selected by the verified token. A ws-B token cannot read
ws-A channel history, inbox rows, members, dashboard data, or store rows because the host and store
queries are run in ws-B's namespace.

## Directory

The workspace directory is implemented by `rust/crates/host/src/workspaces/`. It stores
`WorkspaceRecord` rows in the reserved namespace `_lb_workspaces`, table `workspace`:

```text
WorkspaceRecord {
  ws: string,
  name: string,
  kind: "workspace",
  status: "active" | "archived",
  ts: number
}
```

This directory is node-level metadata, not tenant data. It answers "which workspaces does this node
know about?" It does not create or migrate the tenant namespace.

## Verbs and routes

| Operation | Host verb | Gateway route | Gate |
|---|---|---|---|
| List active workspaces | `workspace_list` | `GET /workspaces` | `mcp:workspace.list:call` |
| Register a workspace | `workspace_create` | `POST /workspaces` | `mcp:workspace.create:call` |
| Rename / unarchive | `workspace_rename` | `POST /admin/workspaces/{ws}/rename` | `mcp:workspace.delete:call` |
| Archive | `workspace_delete` | `POST /admin/workspaces/{ws}/archive` | `mcp:workspace.delete:call` |
| Purge | `workspace_purge` | `POST /admin/workspaces/{ws}/purge` | `mcp:workspace.purge:call` and `confirm == ws` |

Create and list are used by `features/workspace/WorkspaceSwitcher.tsx`. Lifecycle actions are used by
`features/admin/WorkspacesAdmin.tsx`.

## Lifecycle

- Create upserts the directory row unless the workspace has a purge tombstone.
- List returns active rows only, ordered by logical `ts`.
- Archive flips `status` to `Archived`; the row is hidden from the default list and data is retained.
- Rename writes an active row, so it also unarchives.
- Purge writes a tombstone row with `kind = "__purged__"`. Create and rename do not resurrect a
  tombstoned workspace.

## Tests

The core guarantees are covered by real host and gateway tests:

- host deny coverage in `rust/crates/host/tests/collaboration_test.rs`
- signed session and token-workspace behavior in `rust/role/gateway/tests/gateway_test.rs`
- admin lifecycle behavior in `ui/src/features/admin/WorkspacesAdmin.gateway.test.tsx`

Related docs: `../frontend/collaboration.md`, `../../scope/workspace/workspace-scope.md`, and
`../../sessions/workspace/workspace-docs-session.md`.
