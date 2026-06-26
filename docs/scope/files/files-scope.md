# Files scope — docs/files as shared workspace assets

Status: scope (the ask). Promotes to `public/files/` once the S4 slice ships.

Docs and files are **shared workspace assets** built on the one datastore and the capability
model (README §6.1, §6.12). A document is created private to a user, can be **shared to a team**,
**linked into a channel**, or attached to an inbox item; every read is **capability-checked and
membership-checked**, workspace-first. This is the substrate the AI workflows (S5–S6) stand on —
a scope doc an agent drafts is just a doc asset shared to a team.

> Read with: `../../README.md` §6.1 (datastore + buckets), §6.12 (files + docs/skills as assets),
> §6.6 (caps project onto store), §7 (workspace = tenant), `../store/store-scope.md` (the store
> primitives), `../auth-caps/auth-caps-scope.md` (the grammar), `../tenancy/tenancy-scope.md`
> (the membership graph this scope finally lands), `../skills/skills-scope.md` (skills are the
> same asset shape, grant-gated), `../mcp/mcp-scope.md` (the tool surface).

---

## Goals

- A **document asset**: workspace-scoped, addressed by a stable id, with content + metadata
  (owner, visibility, title, ts), readable only through a capability- and membership-checked verb.
- **Three sharing modes**, all as workspace-first graph relations, never a content copy:
  - **private** to the creating user,
  - **shared** to a team (any team member may read),
  - **linked** into a channel (anyone who may `sub` the channel may read the doc).
- A **non-member is DENIED** a shared doc — the mandatory deny test, at the membership layer
  *below* the workspace wall.
- **Workspace isolation**: workspace B can never read/list workspace A's docs — across store and MCP.

## Non-goals (S4)

- **`DEFINE BUCKET` / S3-backed blob storage.** README §6.12 names SurrealDB buckets as the file
  backing. Buckets are **not available in our embedded build** (`surrealdb` `kv-mem`,
  `default-features=false` — `DEFINE BUCKET` fails to parse; verified). So S4 stores content
  **as a record value** in the workspace namespace — same isolation wall, same one-datastore rule,
  no separate blob service. The bucket/file *API seam* is the abstraction; record storage is the
  physical backing now, and `DEFINE BUCKET` with an S3/GCS backend is a **config swap behind the
  same verb** for cloud-scale blobs (S7). (README §6.12's "currently experimental — validate for
  heavy blob workloads" is exactly this caveat.)
- **Large-blob streaming / chunking / dedup** — content is an inline value at S4 (docs are text:
  scope docs, skill bodies). Streaming put/get is an S7 concern with the real bucket backend.
- **A full RBAC team-admin UI** — teams/membership are records minted in tests/by an admin verb;
  the team-management surface is later.
- **Cross-workspace sharing / federation** — out of scope by design; the wall is the product.
- **Versioning of docs** — docs are mutable-by-upsert at S4. **Skills** carry versions
  (`../skills/skills-scope.md`); docs do not yet.

## Intent / approach

Two layers, deliberately separate (this is the §3.6 "isolation first, then capability" order,
extended one notch *within* the workspace):

1. **The workspace wall (gate 1)** — already structural: every doc record lives in the workspace
   namespace; a `store::read/list` for ws A physically cannot see ws B (README §7). Unchanged.
2. **The capability gate (gate 2)** — a new store surface resource: `store:doc/*:read|write`. A
   token without it cannot reach the doc verbs at all. Same `caps::check` chokepoint as every
   surface; no new check path.
3. **The membership gate (gate 3, NEW — the layer tenancy-scope deferred)** — *which* docs within
   the workspace a principal may read. A `read_doc(principal, ws, id)` verb resolves the doc's
   **visibility** and asks: is the principal the owner (private), a member of the shared team
   (shared), or a `sub`-grantee of a linked channel (linked)? If none → **denied**, even with the
   `store:doc/*:read` capability. The capability says "this actor may use the doc surface"; the
   membership relation says "this actor may see *this* doc."

**Sharing is a relation, not a copy.** Sharing writes a small relation record
(`share:{doc}/{team}` or `link:{doc}/{channel}`), never a second copy of the content. A read
re-resolves the live relations — so revoking a share is deleting one relation, and the doc
instantly stops being visible. This is the graph-membership model README §6.1/§6.11 calls for.

**Why records, not SurrealDB `RELATE` edges, for the relations (S4).** README §6.1/§6.11 model
membership as graph edges (`entity ->tagged-> tag`). `RELATE` is the right long-term shape, but
the membership *decision* at S4 is a handful of point lookups (owner? team member? channel
grant?), which the proven `store::read/list` primitives express directly and testably — without
introducing graph-query surface area before the membership graph has more than one consumer. The
relation records are named so a later `RELATE`-backed projection is a drop-in (a graph edge is
just a relation with traversal); recorded as an open question. **Rejected:** minting a per-(user,
doc) capability at share time (would put membership in the token — the same anti-pattern auth-caps
rejected for the workspace claim: a relation you can revoke beats a grant you must chase down).

## How it fits the core

- **Tenancy / isolation:** gate 1 is the workspace namespace (structural); gate 3 (membership) is
  a *second* isolation layer strictly *within* a workspace — a member of team X cannot read a doc
  shared only to team Y, in the same workspace. This is the layer `tenancy-scope.md` named as
  "when the membership graph lands."
- **Capabilities (deny path):** `store:doc/*:read|write` gates the surface. The deny test: a
  principal **without** the cap is refused; and a principal **with** the cap but **without
  membership** is *also* refused (the gate-3 deny — the mandatory non-member test).
- **Placement:** `either`. Docs are shared workspace data → hub-authoritative, edge read-cache,
  synced via the §6.8 append-style path (the same `(table,id)` idempotent upsert as channel items).
- **MCP surface:** `assets.put_doc`, `assets.get_doc`, `assets.share_doc`, `assets.link_doc`,
  `assets.list_docs` — the same verbs the host exposes, reached identically by UI/agent/extension.
- **Data (SurrealDB):** `doc:{id}` (content + metadata) ; `share:{doc}/{team}` and
  `link:{doc}/{channel}` (relation records) ; `member:{team}/{user}` (team membership). All in the
  workspace namespace via the existing `store::read/write/list`. State only — no motion here.
- **Bus (Zenoh):** none at S4 for the doc itself (a doc is state). A *linked* channel already
  carries its own messages; the doc is read on demand. (A "doc shared" notification is an inbox
  item later, not a bus concern now.)
- **Sync / authority:** hub-authoritative shared data; append-style idempotent apply, same as
  channels (§6.8). Not exercised as a new test at S4 (the channel sync test already proves the
  mechanism for `(table,id)` records); listed as N/A-for-now with a note.
- **State vs motion:** docs are pure state. ✔
- **One datastore:** content lives in SurrealDB as a record — no blob service. ✔
- **Stateless extensions:** the doc verbs are host verbs; no extension holds doc state. ✔

## Example flow

1. Ada (`user:ada`, ws `acme`, cap `store:doc/*:write`) calls `put_doc("scope-x", "draft…")`.
   The host writes `doc:scope-x` with `owner=user:ada`, `visibility=private`. Only Ada can read it.
2. Ada calls `share_doc("scope-x", team="engineering")`. The host writes `share:scope-x/engineering`.
3. Ben (`user:ben`, member of `team:engineering`, cap `store:doc/*:read`) calls
   `get_doc("scope-x")`. Gate 1 ws=acme ✔, gate 2 cap ✔, gate 3: Ben ∈ engineering ∧ doc shared to
   engineering → **read returns the content.**
4. Cleo (`user:cleo`, *not* in engineering, holds `store:doc/*:read`) calls `get_doc("scope-x")`.
   Gates 1+2 ✔, gate 3: not owner, not in a shared team, no linked channel grant → **DENIED.**
5. Ada calls `link_doc("scope-x", channel="eng-general")`. Now anyone with `bus:chan/eng-general:sub`
   may also read the doc (the channel-link path), without being in engineering.

## Testing plan (mandatory categories apply)

- **Capability-deny (mandatory, §2.1):** `assets/tests/doc_deny_test` —
  (a) no `store:doc/*:read` cap → `get_doc` denied; (b) **non-member** with the cap → `get_doc`
  denied (the gate-3 membership deny — the S4 exit-gate "a non-member is DENIED").
- **Workspace-isolation (mandatory, §2.2):** `assets/tests/doc_isolation_test` — ws-B principal
  cannot `get_doc`/`list_docs` a ws-A doc (store layer); and `host` MCP-level: a ws-B tool call for
  a ws-A doc is refused before resolve. Across **store + MCP**, per the prompt.
- **The share→link→read happy path (the exit gate):** a private doc is shared to a team and a team
  member reads it; the same doc linked into a channel and a channel `sub`-grantee (non-team-member)
  reads it. Revoking the share removes visibility.
- **Sync:** N/A as a *new* test — doc records are `(table,id)` upserts the channel sync test already
  covers; noted, not re-proven.

## Risks & hard problems

- **Bucket-vs-record divergence.** The verb signature must be the one a real bucket backend can
  satisfy (put/get by id, list, content-as-bytes), so the S7 swap is config-only. Designing the
  verb around the *record* shape would leak the temporary backing. Mitigate: the verb takes/returns
  opaque content, never a SurrealDB-specific type.
- **Membership resolution cost.** Resolving visibility on every read is N point lookups. Fine at
  S4 scale; a `RELATE` graph traversal (or a cached projection) is the scale answer — flagged.
- **Three-gate ordering must be exact.** Gate 1 (ws) before gate 2 (cap) before gate 3
  (membership) — a membership check that ran *before* the workspace check would be a leak. The
  verb must call `caps::check` (ws+cap) first, then membership; tested by the isolation case.

## Status note (shipped S4)

Shipped: the three-gate doc read, share→team / link→channel relations, owner-only share, the
`assets.*` MCP bridge, and the UI `DocView`. Content is a **record** (not `DEFINE BUCKET` — not in
our `kv-mem` build; see `../../debugging/store/define-bucket-unavailable-in-kv-mem-build.md`).
See `../../sessions/files/shared-assets-session.md` + `../../public/files/files.md`.

## Open questions

- **`RELATE` vs relation-records** for share/link/membership — records at S4; move to `RELATE`
  when the membership graph has a second consumer (channels ACLs, tags). The record names are
  chosen to make that a projection swap.
- **Team-membership authority** — who mints `member:{team}/{user}`. S4: a host verb / test fixture;
  a `teams.add_member` admin MCP tool with its own cap is the follow-up.
- **Doc content type & size** — text/inline at S4; when binary/large blobs arrive, that's the real
  `DEFINE BUCKET` cutover (S7) — measure then.
- **Doc versioning** — docs are upsert-mutable now; if an audit trail is needed, docs adopt the
  skill version model (`../skills/`). Decide when the first doc-edit-history consumer exists.
- **"Doc shared" notification** — an inbox item on share, so the recipient team sees it. Deferred
  to when the inbox UI surfaces non-channel items.

## Related

- README `§6.1` (datastore/buckets), `§6.12` (files + docs/skills as assets), `§6.6` (caps project
  onto store), `§7` (tenancy), `§6.8` (sync authority).
- Sibling scopes: `../store/store-scope.md`, `../auth-caps/auth-caps-scope.md`,
  `../tenancy/tenancy-scope.md` (membership graph), `../skills/skills-scope.md`,
  `../mcp/mcp-scope.md`, `../extensions/extensions-scope.md` (install records share the asset slice).
- Public (on ship): `../../public/files/files.md`.
