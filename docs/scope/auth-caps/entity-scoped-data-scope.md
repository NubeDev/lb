# Auth-caps scope — entity-scoped data (row-level reach over federated data)

Status: scope (the ask). Promotes to `public/auth-caps/` once shipped.

> Read with: `entity-scoped-grants-scope.md` (the scoped grant record and `scope_filter` — this
> scope is its first **core** enforcer), `nav-reach-scope.md` (a handed menu decides reach — this
> scope extends that from boards to entities), `access-model-scope.md` (row-level redaction was a
> non-goal there; this scope takes it on for federated reads), `../datasources/datasources-scope.md`
> (the federation path this scope gates), `../testing/testing-scope.md` §2.

A workspace is lb's only hard wall today. Inside one, any principal holding
`mcp:federation.query:call` — which the viewer floor carries — can run any single SELECT against a
federated datasource as its one DB role: a viewer handed a one-site menu can still read every site
by editing a URL, changing a dashboard variable (variables are interpolated in the browser), or
`POST /mcp/call` with hand-written SQL. Insights and cases have no per-record check either. A
product that serves many customer groups from one workspace (the first ask: ~214 sites, 20-30
groups, "group1 sees its 10 sites, admins see all", no duplicated boards) cannot be built on that.

We want **entity-scoped data**: a principal's reach over a named entity table (e.g. `site`) is
resolved on the server, and every read path that can return that entity's data is narrowed to it —
federated SQL, insights, cases — with the SAME boards and menus serving every group.

## Goals

- **Same boards, narrowed data.** A restricted principal runs the exact panels an admin runs; the
  host narrows the rows. Totals, maps, pickers and charts shrink on their own.
- **Server-resolved, never client-supplied.** The entity set is computed from server state (menus,
  grants), never from URL, variables or tool args.
- **Pluggable sources.** One trait, `EntityScopeSource`; the set is the UNION of the sources a
  datasource's policy enables:
  - `nav` — entities marked on the principal's **handed** menu (`NavItem.entity: {table, id}`),
    walked at any depth; a personal pick never widens (valve 2 of `nav-reach-scope.md`).
  - `grant` — the existing entity-scoped grant, `grants.assign {subject, cap: "data:<table>:read",
    scope: {kind:"ids", table, ids}}`, read via `scope_filter`.
  - later sources (e.g. tag rules) are one file each; enforcement never changes.
- **Fail closed.** Enforcement on + restricted principal + empty or failed set = no rows, never all.
  Workspace admins and `Principal::system` are unrestricted.
- **Rule 10.** lb stores an opaque `(table, id)` and a per-datasource policy; it never learns what a
  "site" is.

## Non-goals

- A second hard wall replacing workspaces (tenancy stays `tenancy-scope.md`).
- Scoping lb-native series/store data (federated data only; `mirror` is refused for restricted
  principals so federated rows cannot be copied out of scope).
- Column-level redaction.

## Design

### Policy (per datasource, admin-only)
`datasource_row_policy:{ws}:{source}` — `{enforce, scope_sources: ["nav","grant"], entity_table,
tables: {<name>: {kind: "point"|"entity", columns...}}, entity_key: {table, key_col, value_col,
match}, extra_functions: []}`. Verbs `federation.row_policy_set/get`, admin-only cap.

### Resolution
`host/src/authz/entity_scope.rs::entity_scope(node, principal, ws, table) -> EntityScope::{All,
Ids}`; live (not minted into the token), ~30 s cache cleared on nav/grant/team/member events; keyed
by `owner_sub()` so API keys, agents, reminders and report fires inherit their owner's scope.

### Enforcement
1. `federation.query` (the one choke point for `query.run`, rules, `viz.query`, channel worker):
   restricted + no policy → deny; else `row_scope {policy, ids}` rides the sidecar input (and so its
   cache key).
2. Sidecar `row_policy::rewrite` after macro expansion (sqlparser `VisitorMut`): every relation must
   be a policy table and is replaced by `(SELECT * FROM rel WHERE <pred>) AS alias`; CTEs may not
   shadow policy tables; `information_schema`, `pg_catalog`, table functions rejected; functions
   deny-by-default (allow-list + policy extras); predicates built as AST literals.
3. `schema` narrowed to policy tables; `sample`/`profile*`/`mirror`/writes refused when restricted.
4. `viz.query` response cache key includes the scope hash.
5. Insights (`list`/count/facets/`get`/`watch`/occurrences/comments/ack/assign/resolve,
   subscriptions) and cases (`list`/facets/scorecard/group/`get`/comment/assign/assignees/breach
   notify) narrowed by the entity tag / facet.

## Boundaries and known limits (reviewed 2026-09-22)

- **Who is unrestricted.** A workspace admin, meaning a principal holding any admin-marker cap
  (`nav/admin_lens.rs`: `teams.manage`, `grants.assign`, `workspace.delete`, `ext.uninstall`,
  `apikey.manage`, `webhook.manage`, `members.manage`). A custom role that hands a member one of those
  caps also lifts them out of the row policy — grant them knowingly.
- **Menus are access.** With the `nav` source on, `nav.save`/`nav.share`/`nav.set_default` grant
  data. They sit in the admin bundle; a custom role carrying them can hand any entity to any team.
- **A derived principal** (an extension backend, the agent) reads with its OWNER's scope and standing.
  A reactor run with no owner (`node:reactor`) is restricted and, holding no menu, reads nothing; a
  rule or reminder scheduled by a person runs as that person.
- **API keys** reach only what a workspace-default menu marks (no team edges, no user grants).
- **Raw store reads** (`store.query`, member tier) are refused for a restricted caller: an insight or
  case table cannot be narrowed row by row.
- **Freshness.** Scopes and policies are cached 30 s per workspace and dropped on every write that can
  change access (menu save/share/unshare/delete/default, team membership, grants, policy), so a change
  takes effect at once for callers on this node.
- **Resource use.** Allow-listed set-returning functions (`generate_series`, `repeat`) are unbounded;
  a `statement_timeout` on the datasource DSN is the backstop, not the rewrite.

## Test plan (real store, no mocks)
- Rewriter attack suite: aliases, CTE shadowing, recursive CTE, subqueries (WHERE/SELECT/LATERAL),
  UNION, `query_to_xml`, `set_config`, `current_setting`, `dblink`, `pg_catalog`,
  `information_schema`, schema-qualified and quoted names, quotes in ids, empty set.
- Embed (`host/tests/entity_scoped_data_test.rs`): a team whose handed menu marks entity A reads
  only A with the SQL an admin uses; a principal handed no menu reads nothing; a personal pick or
  "show all pages" neither widens nor zeroes; two team menus union; insights, cases and `store.query`
  follow the same scope; an unenforced policy changes nothing.

## Rollout (first consumer: rubix-ai ESR)
Deploy with no policy (no-op) → dry-run every board's SQL through the rewriter and tune the policy →
mark entities on menus → enforce.

## Open questions
- Multi-team principals: tier 2 picks the first team-shared menu; union across teams later?
- Channels carrying query results to mixed-scope members: deny or re-run per viewer.
