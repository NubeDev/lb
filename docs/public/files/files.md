# Files — docs as shared workspace assets (shipped S4)

The trimmed truth of what shipped. Full design: `../../scope/files/files-scope.md`; session:
`../../sessions/files/shared-assets-session.md`.

A **document** is a workspace-scoped asset with content + metadata, read only through a
**three-gate** verb (README §6.12). Sharing is a live graph relation, never a content copy.

## The three gates (in order)

1. **Workspace** (gate 1, structural) — every doc record lives in the workspace namespace; a read
   for ws A physically cannot see ws B (README §7).
2. **Capability** (gate 2) — `store:doc/{id}:read|write` via the shared `caps::check` chokepoint.
   No grammar change was needed: the auth-caps `store` surface already covers it.
3. **Membership** (gate 3, the layer tenancy deferred) — *which* doc, within the workspace, a
   principal may read: **owner**, or a **member of a team it's shared to**, or a **`sub`-grantee of
   a channel it's linked into**. None → denied, even with the read capability.

## Storage — content as a record, not a bucket (yet)

README §6.12 names SurrealDB **buckets** as the file backing. Buckets are not available in the
embedded `kv-mem` build (`DEFINE BUCKET` fails to parse — see
`../../debugging/store/define-bucket-unavailable-in-kv-mem-build.md`), so content is stored **as a
record value** in the workspace namespace — same one-datastore, same isolation wall, no blob
service. The verbs take/return **opaque content**, so swapping to a real `DEFINE BUCKET` over
S3/GCS at cloud scale (S7) is config behind the same verb, not an API change.

## Verbs (host)

- `put_doc(store, principal, ws, id, title, content, ts)` — create/update; owner forced to the
  caller. Requires `store:doc/{id}:write`.
- `get_doc(store, principal, ws, id)` — the three-gate read.
- `list_docs(store, principal, ws)` — the caller's own docs.
- `share_doc(store, principal, ws, id, team)` / `link_doc(…, channel)` — owner-only; write the
  `share`/`link` relation. Revoke = delete the relation (the doc instantly stops being visible).
- `add_member(store, principal, ws, team, user)` — populate a team (admin-ish; gated by the doc
  write cap at S4).

## Over MCP

All verbs are reachable through the one MCP contract as `assets.<verb>` via `call_asset_tool`: the
**MCP gate** (`mcp:assets.<verb>:call`, workspace-first) runs first, then the verb adds its own
store + membership gate. Two independent surfaces — an MCP grant never bypasses the store check.

## Sharing relations

One generic `(kind, a, b)` edge backs every sharing fact: `share` (doc→team), `link`
(doc→channel), `member` (team→user). A read re-resolves them live; revoke is one delete. Records
at S4 (a `RELATE` graph projection is a later optimization).

## Tested

Capability-deny (no cap, and **non-member with the cap**), workspace-isolation (store + MCP),
share→read, link→channel-read, owner-only share — `host/assets_doc_test`,
`host/assets_isolation_test`, `host/assets_mcp_test`, `assets/tests/*`, `ui DocView.test.tsx`.

**Exit-gate clause met:** a doc private to a user can be shared to a team and linked into a
channel; a non-member is denied.
