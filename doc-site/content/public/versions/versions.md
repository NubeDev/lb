# Version history — the last N snapshots of every dashboard, flow, and rule

> **Unreleased.** The engine is built and green in the working tree but is **not yet in a `node-v*`
> tag**, so no released node serves these verbs. This page describes the shipped-in-tree surface so
> the contract is written down once; treat it as authoritative for the code, not for any tag.
>
> - The ask: `docs/scope/versions/entity-version-history-scope.md` (`NubeDev/lb#112`)
> - The working log: `docs/sessions/versions/entity-version-history-session.md`
> - The UI half: `NubeIO/rubix-ai` → `docs/scope/frontend/version-history-scope.md`

Every save used to destroy the previous record. Now the newest **N** full snapshots of each
dashboard, flow, and rule are kept automatically — no caller opts in, no save path was touched — and
any of them can be restored.

This is **not undo**. Undo is a per-actor, linear stack of *your* last actions. Version history is a
per-entity, addressable ring of *the entity's* past states, visible to anyone who can read it. They
compose: a restore is itself undoable.

## The verbs

| verb | who | what |
|---|---|---|
| `versions.list {kind, id, limit?}` | viewer | the ring, newest-first, **metadata only** |
| `versions.get {kind, id, version_id}` | viewer | one snapshot's full content |
| `versions.restore {kind, id, version_id, now?}` | member + the kind's save cap | make that version live again |
| `versions.config.get` | viewer | how many versions this workspace keeps |
| `versions.config.set {cap?, per_kind?}` | **admin** | change it (merges; `1..=100`) |

Gateway routes mirror them one-to-one: `GET /versions/{kind}/{id}`,
`GET /versions/{kind}/{id}/{version_id}`, `POST /versions/{kind}/{id}/{version_id}/restore`,
`GET|PUT /versions/config`. Workspace and principal come from the token, never the path.

`kind` is `dashboard`, `flow`, or `rule`. A ring row carries
`{ version_id, kind, entity_id, entity_rev, entity_version, tool, actor, ts, hash, is_head }`;
`ts` is unix **milliseconds** and `entity_version` is the kind's own counter when it keeps one (a
flow's `v12`), otherwise `null`.

## What it guarantees

- **Automatic.** Capture happens at the depth-0 dispatch chokepoint — the same seam undo uses — so
  every successful `dashboard.save` / `flows.save` / `rules.save` produces a version, whoever called
  it and however (UI, agent, MCP, a pack).
- **Bounded.** Default 20 per entity, admin-adjustable per workspace and per kind, hard-clamped
  `1..=100` at the node. Insert and trim happen in one transaction, so concurrent saves cannot make
  a ring overgrow.
- **No wasted slots.** An identical re-save is deduped — save metadata the record stamps on itself
  (a dashboard's `updated_ts`, a flow's counter) is excluded from the comparison but kept in the
  snapshot.
- **Restore is a forward action.** It re-dispatches the entity's own save verb with the snapshot, so
  it runs every validator, capability check, ownership rule, audit hook, and cache invalidation the
  save already has. A snapshot that no longer validates is **refused, not written**.
- **Restore survives delete.** There is no capture on delete, so the ring outlives the entity and
  restoring the head re-creates it.
- **History is append-only.** A restore *appends* a new head equal to the version it restored;
  nothing is ever rewritten, and a restore can itself be undone or re-restored.
- **Last-writer-wins on a race.** Unlike undo — which refuses when the record drifted, because an
  undo asserts a state it observed — a restore asserts an intent ("make it look like this again").
  Nothing is lost by racing: the pre-restore state is the ring's previous head.

## Security

- **Workspace is the wall.** Rings are ordinary workspace-scoped records; a version id minted in one
  workspace does not resolve in another.
- **No escalation.** `versions.restore` requires the caller to hold the kind's own save cap, checked
  *before* the snapshot is read — so a refusal cannot be used to probe which versions exist.
- **Denials are opaque.** A refused `versions.get` is indistinguishable from one for an entity that
  does not exist.
- **Secrets never enter a snapshot.** A structural guard (`lb_store::snapshot_safety`) refuses to
  snapshot the secret plane at all, and refuses any record carrying secret-shaped material —
  *refuses*, never redacts, because a redacted snapshot would look restorable and would write `***`
  over a live credential. A refused capture costs one version and is logged loudly; the user's save
  is never affected.

## Adding a kind

A kind is a **data row** in `crates/host/src/versions/plan.rs`:
`(kind → table, save_tool, id keys, counter field, hash-ignore)`. Nothing downstream matches on a
kind name — the gateway treats it as an opaque path segment, the tool descriptors derive their enum
from the table, and undo reaches it through the same table. Extension-declared kinds are a named
follow-up; the seam is shaped for them.
