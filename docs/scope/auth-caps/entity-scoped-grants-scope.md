# Auth-caps scope — entity-scoped grants (row-level reach inside a workspace)

Status: scope (the ask). Promotes to `public/auth-caps/` once shipped.

> Read with: `auth-caps-scope.md` (the cap grammar + the three gates),
> `authz-grants-scope.md` (the durable grant store + `resolve_caps` — this scope extends
> *that* record and *that* resolver), `access-model-scope.md` (team-as-unit + asset
> visibility — the sibling mechanism for **shell assets**; this scope is the analog for
> **extension domain records**), `../testing/testing-scope.md` §2.

Today a capability answers "may this principal call this **tool** in this **workspace**" —
it says nothing about **which records**. Any feature where a member may reach only a
*subset* of a table's rows has no platform support: a guardian who may read only *their*
children's daily logs (the childcare product's defining invariant), a technician who may
ack only their assigned sites' alarms, a client who may see only their own project. Every
extension is currently forced to hand-roll that filter inside its own verbs — N re-implementations
of the most leak-prone check in the system, invisible to the Access console, untestable by the
platform. We want **entity-scoped grants**: a grant that carries a **resource selector**, resolved
by the same grant store, checkable at the same wall.

## Goals

- **A grant can name resources.** Additive field on the existing grant record:
  `scope: { table: "child", ids: [...] } | { table, tag } | All` — default `All`
  (today's behavior, zero migration).
- **One host check.** `caps::check_scoped(principal, cap, resource_ref) -> Allow|Deny`,
  and a **query-side filter** `caps::scope_filter(principal, cap, table) -> All | Ids(..)`
  so `list` verbs get "which rows" in one call instead of post-filtering.
- **Extensions consume it via the host callback ABI** (an SDK function, e.g.
  `lb::authz::scope_filter(...)`), so an extension verb *asks* the wall instead of
  re-implementing it. The id/table are **opaque data** to the core (rule 10).
- **Administered as data:** scoped grants ride the existing `grant` CRUD, show in the
  Access console with their selector, and are revoked/re-derived like any grant.
- **Deriveable by extensions:** a domain event (a guardianship edge linked/unlinked) can
  create/remove scoped grants through the normal granted `grants.*` verbs — the extension
  owns *when*, the core owns *what it means*.

## Non-goals

- **Not ABAC / a policy language.** No expression evaluation, no attribute predicates —
  a selector is ids or a tag, resolved to ids. (Rejected: a rules/CEL-style policy engine —
  enormously more surface, unauditable in the console, and every past scope that smelled
  like a policy engine was cut for the same reason; see `access-console-scope.md` risks.)
- **Not replacing asset visibility.** Dashboards/docs keep the shipped share-to-team
  model; this is for extension-owned domain records the asset system never sees.
- **Not cross-workspace anything.** The workspace wall is untouched; a selector narrows
  *within* it, only ever **subtractive** (`granted = caps ∩ scope`), never widening.

## Intent / approach

Extend, don't invent: the `authz-grants` record gains an optional `scope` selector; `resolve_caps`
resolves a principal's caps as today **plus** a per-cap scope union (a principal may hold the same
cap through several scoped grants — union of selectors; any `All` grant wins). The check API and
filter API are thin reads over that resolution, cached with the same freshness levers
(`builtin-role-freshness-scope.md` applies here too — a stale scope union is a leak or a lockout).

**Rejected alternative — "a team per entity" over asset visibility:** model each child/site/project
as an asset shared to a micro-team of its guardians. It reuses shipped machinery but explodes teams
(one per child × thousands), makes the Access console unreadable, forces every domain record to
become an "asset," and still gives `list` verbs no query-side filter. The selector-on-grant keeps
one grant row per (principal, cap, scope) and reads naturally in the console.

## How it fits the core

- **Tenancy / isolation:** selectors are workspace-scoped like the grants that carry them;
  a selector can never name a record in another workspace (checked at write).
- **Capabilities:** pure narrowing of the existing grammar. Deny path: `check_scoped` on a
  record outside the union → the same 403 shape as a missing cap; `scope_filter` → `Ids([])`
  (an empty list, not an error) so lists degrade to empty.
- **Placement:** either role; resolution is store-local like `resolve_caps` today.
- **MCP surface (§6.1):** no new tools for callers. `grants.create/update` accept the
  additive `scope` field (existing verbs); the host exposes `check_scoped`/`scope_filter`
  to extensions via the SDK host-callback, and the Access console reads the resolved unions
  through the existing `resolve_caps` surface (extended payload). Live-feed/batch N/A.
- **Data (SurrealDB):** one additive field on `grant`; no new tables. State only.
- **SDK/WIT impact — flagged loudly:** one **additive** host-callback pair in `lb-ext-sdk`
  (`authz.check_scoped` / `authz.scope_filter`). Additive WIT, no `WORLD_MAJOR` bump.
- **No mocks:** tests run the real store + resolver with seeded grants.

## Example flow

1. The childcare extension links guardian Ana ↔ child Leo; on that domain event it calls the
   granted `grants.create` with `{subject: Ana, cap: "mcp:care.log.list:call", scope: {table:
   "child", ids: ["child:leo"]}}` (and siblings for read/watch verbs).
2. Ana calls `care.log.list`. The verb asks `scope_filter(ana, "mcp:care.log.list:call",
   "child")` → `Ids(["child:leo"])` → one indexed query. Sam (edges to Leo **and** Mia) gets
   `Ids([leo, mia])` from the union of two scoped grants.
3. Ana tries `care.log.get` on one of Mia's entries → `check_scoped` → deny → 403.
4. The edge is unlinked → the extension removes the grants → next resolution excludes Leo;
   the Access console showed the selector the whole time.

## Testing plan

Mandatory: **capability-deny** (cap present but record outside scope → 403), **workspace
isolation** (selector naming a foreign-ws id rejected at grant write; resolution never
crosses), plus: union-of-grants (two selectors merge), `All`-wins, empty-scope lists return
empty not error, revoke → immediate deny (freshness), console renders selectors. Regression
harness seeds two "families" and asserts every read/list/watch verb of a fixture extension.

## Risks & hard problems

- **Resolution cost on hot list paths** — the scope union must be one cached read, not a
  per-row check; the filter API exists precisely so verbs push ids into the query.
- **Freshness** — a stale cached union after unlink is a data leak. Reuse the
  invalidate/re-mint levers; a deny-after-revoke test is mandatory.
- **Selector sprawl** — thousands of id-selectors per subject degrade; the `tag` selector
  form exists for cohort cases. Measure before optimizing.

## Open questions

- ✅ Selector forms for v1: `ids` only (tag deferred — no real cohort caller yet). The `Scope`
  enum is designed so `tag` is an additive variant later.
- ✅ `scope_filter` returns ids (not a WHERE fragment) — keeps the core out of query-string
  business. The caller pushes ids into its own indexed query.
- ✅ Watch verbs: filter-at-emit in the extension for v1 (no scoped subscription helper).

## Related

`authz-grants-scope.md` · `access-model-scope.md` · `access-console-scope.md` ·
`builtin-role-freshness-scope.md` · `../extensions/extensions-scope.md` (host-callback ABI) ·
README §3 rule 5/10, §6.6 · first consumer: `cc-app` `docs/scope/care/care-authz-scope.md`.
