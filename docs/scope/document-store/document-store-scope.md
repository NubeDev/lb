# Document-store scope — a reusable markdown document store

Status: scope (the ask). Promotes to `public/document-store/document-store.md` once shipped.

A **reusable markdown document store**: workspace-scoped markdown documents kept in the one
datastore, with first-class **images, attachments, and internal links**, a **save** action that
participates in undo/redo, full **CRUD over MCP**, and sharing to a **user, team, or the whole
workspace**. It is not a one-off UI feature — it is a *substrate* that **extensions** (via the
host-callback ABI) and the **doc-site** consume the same way the first-party shell does. This is
the README §6.12 "docs/files as shared workspace assets" goal made concrete; the **new** surface
over the shipped S4 asset model is markdown content plus binary assets and the link graph.

> Read with: README §6.12 (files + docs/skills as assets — the original goal), §6.1 (datastore +
> buckets), §6.6 (caps project onto store), §7 (workspace = tenant). Sibling scopes:
> `../files/files-scope.md` (the shipped S4 three-gate doc asset this builds on),
> `../skills/skills-scope.md` (versioned asset shape), `../auth-caps/authz-grants-scope.md`
> (user/team grant model), `../undo/undo-scope.md` (the save↔undo seam),
> `../extensions/host-callback-scope.md` (how a guest reaches host tools — the reusability path),
> `../frontend/dashboard-scope.md` (the same `private|team|workspace` sharing, extended with `user`).

---

## Goals

- A **markdown document** as a workspace asset: stable id, `body` (raw markdown) + metadata
  (owner, title, visibility, `updated_ts`, `rev`), read/written only through a capability- and
  membership-checked verb. Built on the shipped S4 `doc:{id}` asset — markdown is a typed
  `content_type`, not a new substrate.
- **Binary assets** — images and file attachments — stored in the **SurrealDB file store**
  (README §6.12 buckets), referenced from markdown by a stable workspace URI and fetched through a
  gated verb. This is the part S4 deferred (no `DEFINE BUCKET` in the `kv-mem` build); it lands now
  on the persistent engine, behind the same opaque-content verb seam so the physical backing stays
  config.
- **Internal links** as a first-class, queryable relation: doc→doc links and doc→asset embeds are
  stored as edges so "what links here" (backlinks), broken-link detection, and orphan-asset GC are
  possible — and so a link **never widens access** (the target is re-gated at read).
- **Sharing to a user, a team, or the workspace** — `Private | Team | User | Workspace` — reusing
  the shipped S4 `(kind, a, b)` relation edge (adding the `user` subject; the edge already supports
  it). A non-member is **denied**, the mandatory gate-3 test.
- **Save participates in undo/redo** — a markdown save is a single `write_tx` mutation, so the
  shipped undo journal (`../undo/`) captures its before/after image for free; undo restores the
  prior body.
- **Reusable, not bespoke** — the store is a host service + MCP family with **no domain schema**;
  an extension stores its markdown/assets through the host-callback ABI under `caller ∩ grant`,
  and the doc-site authors its pages into the same store. One store, many consumers.

## Non-goals (v1)

- **Anonymous / public read + public doc-site *serving*.** Making a doc readable with no token —
  the doc-site's public face — deliberately breaks the workspace wall and needs its own threat
  model + an unauthenticated gateway route. Scoped as a **deferred slice** (see "Deferred: public
  publishing"), not v1. v1 makes the store reusable by extensions and by the doc-site's
  **authoring** side; serving pages to the anonymous internet comes later.
- **Live collaborative co-editing** (CRDT, cursors, presence-on-the-doc). The model is **save
  button + undo/redo**, not real-time co-edit. A doc is state; multi-writer convergence is a large,
  separate concern.
- **Server-side markdown→HTML rendering.** The store holds **raw markdown** and *resolves* links
  and assets to gated references; turning markdown into HTML is the **consumer's** job (the React
  renderer, the doc-site's MDX). Keeping rendering out is what keeps the store reusable and
  domain-free. (Sanitization is therefore a consumer contract — flagged in Risks.)
- **Large-blob streaming / chunking / dedup.** Binary assets are bounded inline/bucket values in
  v1; streaming put/get is the same later concern as the rest of §6.12's bucket story.
- **Full version-snapshot history.** Docs are upsert-mutable; recent history comes from the undo
  journal. An immutable snapshot model (the broader document-store vision below) adopts the skills
  version shape when the first audit-trail consumer exists.
- **PDF/spec/review-pack workflows, approval states, outbox publishing.** The original
  document-store vision (scope drafting → review → approval → outbox) composes *on top* of this
  substrate via the shipped inbox/outbox/jobs; those are later slices, not this one.

## Intent / approach

**Build on the shipped S4 asset, don't fork it.** S4 already ships the hard part — a workspace doc
with the **three-gate** read (workspace → capability → membership), sharing as a live `(kind,a,b)`
relation, and the `assets.*` MCP bridge (`../files/files-scope.md`). This scope is **additive** over
that, in three moves:

1. **Type the content.** A doc carries `content_type` (`markdown` | the legacy opaque `text`).
   Markdown docs are the same `doc:{ws}:{id}` record with `body` holding raw markdown. No new
   table, no new gate — the three-gate read is unchanged.

2. **Land the file store for binaries.** Images and attachments are **not** markdown text, so they
   get their own asset record `asset:{ws}:{id}` whose bytes live in the SurrealDB **file/bucket**
   path README §6.12 names. The verb takes/returns **opaque bytes** (the same seam S4 used for doc
   content), so whether the physical backing is `DEFINE BUCKET` (if the persistent engine supports
   it — open question) or a record value, swapping is config, not an API change. A binary asset is
   read through the same three gates as a doc.

3. **Make links a relation.** When a markdown body references another doc (`[[doc:id]]`) or embeds
   an asset (`![alt](lb-asset://id)`), the save writes a `link` (doc→doc) or `embed` (doc→asset)
   edge — the *same* generic relation record as S4 `share`/`link`. This gives backlinks, broken-link
   detection, and orphan GC by querying edges, and — crucially — lets a **read re-gate the target**
   so a link can never widen access.

**Reusability is the host-callback ABI, not a new mechanism.** Because the store is plain host
verbs behind the one MCP contract, a WASM or native extension reaches it through the shipped
`build_call_context` (`caller ∩ install-grant`, two-direction deny, no widening — the same path
flows/extension-nodes use). An extension that needs to persist markdown + images (release notes, a
runbook tool, the doc-site authoring) calls `assets.put_doc` / `assets.put_asset`; its durable
content lives in the store, not the instance (rule 4, stateless extensions). The store holds **zero
extension-specific schema** — that domain-freeness *is* the reusability.

**Why extend `assets.*` rather than coin a fresh `docs.*` family.** The S4 `assets.*` verbs already
ship docs and are the one asset surface; adding markdown as a typed content and binaries as sibling
verbs keeps a **single** asset model that every consumer already knows. **Rejected:** a parallel
`docs.*` family — it would split the asset substrate in two, churn the shipped S4 surface, and force
every existing caller (and the registry/skill assets that share the relation table) to learn a
second vocabulary for the same three-gate read.

## How it fits the core

- **Tenancy / isolation:** `doc:{ws}:{id}` and `asset:{ws}:{id}` live in the workspace namespace
  (gate 1, structural); membership (gate 3) is the *within-workspace* layer — a `User`-shared doc is
  invisible to everyone but that user, a `Team`-shared doc invisible outside the team. Unchanged from
  S4, extended only by the `user` subject.
- **Capabilities (deny path):** the surface caps `store:doc/{id}:read|write` (shipped) gate markdown
  docs; new `store:asset/{id}:read|write` gate binaries. Plus the MCP caps per verb
  (`mcp:assets.put_doc:call`, `mcp:assets.put_asset:call`, `mcp:assets.get_asset:call`,
  `mcp:assets.delete_doc:call`, `mcp:assets.share_doc:call`, `mcp:assets.link_doc:call`,
  `mcp:assets.backlinks:call`). **Deny:** no cap → refused; cap but **not owner/shared-to** → refused
  (gate-3); and a **linked/embedded** target the reader lacks access to → refused at the target read,
  the link itself is not a backdoor.
- **Placement:** `either`. Hub-authoritative shared data, edge read-cache, synced via the §6.8
  append-style `(table,id)` idempotent upsert — the same path channel items and S4 docs use. Binary
  assets are `(table,id)` records too (streaming is the deferred bucket concern).
- **MCP surface (API shape, §6.1):**
  - **CRUD:** `put_doc` (create/update markdown — the **save**), `delete_doc` (soft-delete
    tombstone, idempotent), `put_asset` / `delete_asset` (binaries), `share_doc(subject)`
    (`user:` | `team:`), `link_doc(target)` (internal doc link). Each its own MCP tool + cap, one
    responsibility per file (FILE-LAYOUT).
  - **Get / list:** `get_doc`, `list_docs` (filter by tag/visibility, workspace-scoped),
    `get_asset`, `list_assets`, `backlinks(id)` (what links here — the relation query).
  - **Live feed:** **N/A v1.** A doc is state, not motion (rule 3); the editor's model is
    save+undo, not a stream. A "doc updated" signal, if needed, is an inbox item later — not a bus
    subject now.
  - **Batch:** an **export/import** of a doc tree + its assets (doc-site publish, workspace
    migration) is a **job** when large (README §6.10) — deferred; v1 is single-doc `(table,id)` sync.
    Named here so it isn't smuggled into a blocking loop later.
- **Data (SurrealDB):** `doc:{ws}:{id}` (markdown body + metadata), `asset:{ws}:{id}` (binary,
  bucket/record-backed), and the generic `(kind,a,b)` relation for `share` (doc→user|team), `link`
  (doc→doc), `embed` (doc→asset). State only — no motion.
- **Bus (Zenoh):** none. A doc/asset is pure state.
- **Sync / authority:** hub-authoritative; idempotent append-style apply (§6.8). Not a *new* sync
  test — `(table,id)` upsert is already proven for channels and S4 docs; noted.
- **Secrets:** none. No secret material in the store.
- **One datastore / state vs motion / stateless extensions:** content (text and bytes) lives in
  SurrealDB — no blob service (rule 2); docs/assets are state (rule 3); an extension's markdown lives
  in the store, never the instance (rule 4). ✔
- **SDK/WIT impact:** none new — extensions reach the store through the **existing** host-callback
  ABI; no change to the stable plugin boundary.

## Example flow

1. Ada (`user:ada`, ws `acme`, caps `store:doc/*:write`, `store:asset/*:write`) writes a runbook in
   the editor and clicks **Save**: `put_doc("runbook-cooler", title, body="…", content_type="markdown")`.
   The host writes `doc:acme:runbook-cooler` (`owner=user:ada`, `visibility=Private`, `rev=1`) in one
   `write_tx` — the undo journal captures the before-image (empty) / after (rev 1).
2. Ada drags in a wiring photo: `put_asset("cooler-wiring", bytes, mime="image/png")` →
   `asset:acme:cooler-wiring`. The editor inserts `![wiring](lb-asset://cooler-wiring)`; the next save
   writes an `embed` edge `doc:runbook-cooler → asset:cooler-wiring`.
3. Ada references the alarms doc with `[[doc:alarm-matrix]]`; the save writes a `link` edge
   `doc:runbook-cooler → doc:alarm-matrix`.
4. Ada shares to a person, not a team: `share_doc("runbook-cooler", subject="user:ben")` →
   `share` edge (doc→user). Visibility becomes `User`.
5. Ben (`user:ben`, cap `store:doc/*:read`) opens the doc: gates 1+2 ✔, gate 3: shared to `user:ben`
   ✔ → `get_doc` returns the markdown; the renderer calls `get_asset("cooler-wiring")` (gate-3 ✔,
   embedded) → the image renders. The `[[doc:alarm-matrix]]` link resolves: Ben **lacks** access to
   `alarm-matrix` → the link renders as an honest "no access", **not** a leak and **not** a fake.
6. Cleo (cap `store:doc/*:read`, not shared to) opens the doc → gate-3 **DENIED**.
7. Ada edits and re-saves (`rev=2`); she hits **Undo** → the journal restores `rev=1`'s body
   (Reversible — pure state).
8. A `release-notes` **extension** (installed, granted `mcp:assets.put_doc:call` +
   `store:doc/*:write`) calls `put_doc` through the host-callback context (`caller ∩ grant`) to
   persist generated markdown — the *same* store, no new path.

## Testing plan (mandatory categories apply)

- **Capability-deny (mandatory):** no `store:doc/*:write` → `put_doc` denied; no `store:asset/*:read`
  → `get_asset` denied; **non-member with the read cap** → `get_doc` denied (the gate-3 membership
  deny — the load-bearing test).
- **Workspace-isolation (mandatory):** a ws-B principal cannot get/list/save/delete a ws-A doc *or*
  asset — across **store + MCP** (a ws-B tool call for a ws-A id refused before resolve).
- **Sharing happy path + revoke:** a `Private` doc shared to a **user** is read by that user and
  **denied to others**; shared to a **team** is read by a member; **revoke** (delete the edge)
  removes visibility immediately.
- **Link/embed never widens (the new deny test):** a doc links to a doc the reader cannot access →
  the reader sees the link but `get_doc` of the target is **denied** (honest no-access), not a leak;
  an embedded image whose asset the reader lacks → `get_asset` **denied**, the cell renders empty.
- **Binary round-trip:** put an image asset, reference it from markdown, `get_asset` returns it
  **byte-identical**; `backlinks`/`embed` edges resolve.
- **Undo (the save↔journal seam):** save markdown rev1→rev2, **undo** restores rev1, via the *real*
  journal at `write_tx` (no app-side guessing) — and the undo is refused if the record moved under it
  (the conditional predicate, per `../undo/`).
- **Reusability for real (no mocks, CLAUDE §9):** a **real seeded extension install** calls
  `assets.put_doc` + `assets.get_asset` through the host-callback context under `caller ∩ grant` —
  proving an extension uses the store for real, not a UI-only path. Real store, real install record,
  no fakes.

## Risks & hard problems

- **Bucket availability on the persistent engine.** S4 deferred binaries because `kv-mem` can't
  `DEFINE BUCKET`. Whether the shipped **SurrealKV** engine parses/serves `DEFINE BUCKET` is
  unverified — if not, binary bytes stay a **record value** (works, size-bounded) behind the same
  opaque-bytes verb, and the bucket cutover is the same config swap. Verify before committing the
  asset backing; do **not** leak a SurrealDB-specific type through the verb either way.
- **Binary size bound.** An inline record value caps practical attachment size; v1 must **reject**
  over-bound puts with a clear error (not silently truncate) until streaming + real buckets land.
  State the bound explicitly.
- **Access must never widen via links/embeds.** The whole link feature is only safe because the
  target is **re-gated at read** (the per-widget dashboard lesson). A resolver that returned the
  target's content because "the parent doc was readable" is a leak — the deny test guards it.
- **Markdown sanitization is a consumer contract, not a store job.** The store keeps **raw**
  markdown; a renderer that injects it as HTML without sanitizing is an XSS hole. The store must
  **never** store pre-rendered HTML and must document that every renderer sanitizes — flag loudly so
  a consumer doesn't assume the store cleaned it.
- **Dangling links & orphan assets.** A `[[doc:id]]` whose target was deleted, or an `asset` whose
  only embedder was deleted: links resolve to a "broken" state (don't hard-fail the read); orphan
  assets are reference-counted via `embed` edges and GC'd by a **later job**, not eagerly.
- **Membership resolution cost.** Resolving visibility + link targets on every read is N point
  lookups — fine at this scale; a `RELATE` projection (or cache) is the scale answer, same flag S4
  raised.

## Deferred: public publishing (its own slice)

The doc-site's public face and "make this page public" are the **same** deferred mechanism: a
**public, read-only, opt-in-per-doc** path that intentionally breaks the workspace wall. When built,
it is one thing reused by both docs **and** dashboards — a `published`/share-token record + an
**unauthenticated** gateway route (`GET /public/…`) backed by a **public-read** verb run under a
guest/system principal that returns content **only** when the doc's visibility is the explicit
`Public` tier. It needs its own threat model (rate-limit, no enumeration, sanitized render, no
link-widening into private targets) and is **out of v1** by the owner's decision. Named here so the
v1 verb signatures (opaque content, three-gate read) are already shaped to accept a fourth gate
rather than be retrofitted.

## Open questions

- **`DEFINE BUCKET` on SurrealKV** — supported, or stay record-value for binaries? Decides the asset
  backing (config behind the same verb either way). Verify first.
- **Canonical internal-link grammar** — `[[doc:id]]` wikilink vs an `lb-doc://{id}` URI vs plain
  relative links. Pick **one** form the resolver/link-extractor recognizes; the rest render as
  external.
- **Doc hierarchy / nav tree** — flat ids + tags (v1) vs a parent/order tree the doc-site needs for
  navigation. Likely a `nav` doc or a parent field as the doc-site follow-up — decide when the
  doc-site authoring consumer lands.
- **Verb naming confirm** — extend `assets.*` (this scope's choice) vs a fresh `docs.*` family. Lock
  before implementation.
- **Versioning** — upsert + undo-journal recent history now; adopt the skills version-snapshot model
  when an immutable audit trail is first required.

## Related

- README `§6.12` (files + docs/skills as assets — the goal), `§6.1` (datastore/buckets), `§6.6`
  (caps→store), `§7` (tenancy), `§6.8` (sync), `§6.10` (jobs — the export batch).
- Sibling scopes: `../files/files-scope.md` (the shipped S4 three-gate asset this builds on),
  `../skills/skills-scope.md`, `../auth-caps/authz-grants-scope.md` (user/team subjects),
  `../undo/undo-scope.md` (save↔journal), `../extensions/host-callback-scope.md` (extensions reach
  the store), `../frontend/dashboard-scope.md` (the shared `private|team|workspace`+`user` model and
  the per-target no-widening lesson), `../tags/tags-scope.md` (doc discovery).
- Public (on ship): `../../public/document-store/document-store.md`.
</content>
</invoke>
