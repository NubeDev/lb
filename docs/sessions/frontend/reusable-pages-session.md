# Session — reusable pages (template dashboards, instance-as-binding, tag-driven fan-out)

Date: 2026-07-03. Scope: [`scope/frontend/dashboard/reusable-pages-scope.md`](../../scope/frontend/dashboard/reusable-pages-scope.md)
(now **SHIPPED**). Built on the shipped variable system, tags graph, nav builder, and library panels.

## What shipped (end to end, no new tables/verbs/caps — deliberately additive)

1. **`Variable.required: bool`** — additive `#[serde(default)]` on the shipped `Variable`
   (`rust/crates/host/src/dashboard/model.rs`; TS `ui/src/lib/vars/types.ts`). Marks a variable a
   **page parameter**. Round-trips `dashboard.save`/`get`; an old record without the field loads
   unchanged.
2. **`RequiredVarGate`** (`ui/src/features/dashboard/vars/RequiredVarGate.tsx`) — when any `required`
   variable is unbound (no `?var-`, no default), `DashboardView` renders the honest "select a `<label>`"
   gate **in place of the `<Grid>`** — so cells never fire a `$site`-literal query. A "template · N
   params" header badge + a "required (page parameter)" toggle in the variable editor complete the
   authoring affordance.
3. **Nav `dashboard` entries gain `vars`** and a **`template-group`** entry kind
   (`rust/crates/host/src/nav/model.rs`; TS `ui/src/lib/nav/nav.types.ts`). A `template-group` names a
   template `dashboard`, the `var` it binds, and one option source — tag `facets` (common) or a
   `{tool,args}` query (general).
4. **Fan-out expansion lives in `nav.resolve`** — a new one-responsibility resolver
   (`rust/crates/host/src/nav/resolve_template_group.rs`) expands a `template-group` to a `group` of
   per-value `dashboard` instances, each carrying `vars = { <var>: <value> }` → the UI folds that into
   `?var-<var>=<value>`. Capped at `MAX_TAG_GROUP` (50), the tag-group precedent.
5. **The lens holds.** Expansion runs under the caller's caps: the tag-facet source gates on the
   existing `tags.find` cap (a new raw `lb_tags::facet_values` + host `tags_facet_values` wrapper — **no
   new cap**); the query source re-enters the generic dispatcher (`call_tool_at_depth`) under the
   caller's caps. A caller lacking the option-source cap → the whole entry is stripped, no value leaks.
   A caller who cannot **read** the template dashboard → the entry is stripped even holding the option
   cap. The instance URL still gates server-side on the dashboard's own read caps.
6. **Nav builder** (`ui/src/features/admin/nav/NavAdmin.tsx`) — a "One dashboard per ⟨value⟩"
   (template-group) authoring form next to "Dashboards by tag" (tag-group), named side by side per the
   scope's "one mental model" risk; a "Pin vars" field on `dashboard` entries.
7. **Binding precedence carrier** — `NavRail` gains `onSelectDashboard(dashboard, vars)`, wired in
   `RoutedShell` to navigate to the board with `?d` + `?var-<name>=<value>`. The nav link SETS the URL;
   after that the URL is the single source of truth (the shipped model, unchanged).

## "Best long-term" decisions made (the scope's "confirm at build start" / open items)

- **`nav_resolve` / `call_nav_tool` now take `&Arc<Node>`** (was `&Node`) — mirroring the shipped `viz`
  re-entrancy precedent — so the `{tool,args}` option source can re-enter the generic dispatcher
  **under the caller's caps** (no render-path bypass). The alternative (a per-source resolver) would
  fork option resolution and violate rule 10 (branching on the tool). The 7 existing nav-test node
  bindings switched to `Arc::new(Node::boot())` (mechanical).
- **Both option sources implemented, not just the common one.** The scope names the tag-facet source as
  "the common case" and `{tool,args}` as "the general case"; shipping both now (the query source via the
  generic dispatcher) avoids a follow-up and proves the lens on both paths. `enumerate_values` prefers
  `facets` when both slip through; bounds reject authoring both.
- **A template-group enumerates the distinct VALUES of a facet key**, not the tagged entities — so
  "one page per site value". This needed a new distinct-values read; added as a **raw** `lb_tags`
  helper (`facet_values`) + a host wrapper gated on the **existing** `tags.find` cap. No new MCP verb,
  no new cap — the tags MCP surface stays `add/remove/of/find`.
- **`NavItem` now derives `Default`** so the additive fields land without churning every constructor
  (tests + the new resolver use `..Default::default()`).
- **Dev-login gained `mcp:tags.find:call`.** The dev/member cap set already held `tags.add` but not
  `tags.find` — an inconsistency that silently stripped *any* tag-driven resolve (variable Query
  sources, tag-groups, template-groups) for the dev user. Added the paired read cap
  (`rust/role/gateway/src/session/credentials.rs`); it's a member-level read, re-checked server-side.
- **RequiredVarGate treats an empty string / empty multi-list as still unbound** — a multi-select that
  resolves to nothing has nothing to interpolate, so it gates too.

## Tests (all green)

- **Rust** `rust/crates/host/tests/reusable_pages_test.rs` (8/8): `Variable.required` round-trip +
  serde default; template-group facet expansion (one instance per value, `vars` binding, new-tag
  appears / untag removes); **capability deny (mandatory)** — no `tags.find` → entry stripped, no
  values; **the lens (headline)** — unreadable template → stripped even with the option cap;
  **workspace isolation (mandatory)** — ws-B enumerates only ws-B values; pinned-vars round-trip +
  resolve; the `{tool,args}` query source enumerates + denies without the tool cap; bounds reject a
  malformed template-group. Full `cargo test --workspace` green; `cargo fmt` clean.
- **UI unit** `ui/src/features/dashboard/vars/RequiredVarGate.test.tsx` (6/6) — the
  `unboundRequiredVars` predicate across the binding-precedence cases + the gate render. Full
  `pnpm test` green (68 files / 436 tests).
- **UI gateway** `ui/src/features/dashboard/ReusablePages.gateway.test.tsx` (2/2, real spawned
  gateway): a template's `required` var round-trips + the bare template shows the gate (cell `c1`
  absent) then binds and renders (cell present); a `template-group` authored through the **real
  builder** → `nav.get` round-trip → `nav.resolve` fans out one bound instance per distinct site value
  on the SAME dashboard record. `pnpm test:gateway`: the only 2 failures are the pre-existing
  `SystemView.gateway` + `sqlSource.gateway` (known-red on clean master), not this change.

## Files touched

Rust: `dashboard/model.rs` (+`required`), `nav/model.rs` (+`vars`/`var`/`tool`/`args`/`ResolvedItem.vars`,
`Default`), `nav/bounds.rs` (+`template-group` kind + validation), `nav/resolve.rs` (`&Arc<Node>`,
template-group arm, `vars` through), `nav/resolve_template_group.rs` (new), `nav/tool.rs` (`&Arc<Node>`),
`nav/mod.rs`, `tags/values.rs` (new `facet_values`), `tags/{lib,verbs,mod}.rs` + host `lib.rs`
(`tags_facet_values`), `role/gateway/.../credentials.rs` (+`tags.find`).
UI: `lib/vars/types.ts`, `lib/nav/nav.types.ts`, `features/dashboard/vars/RequiredVarGate.tsx` (new) +
`.test.tsx`, `DashboardView.tsx`, `vars/VariableEditor.tsx`, `admin/nav/NavAdmin.tsx`,
`shell/NavRail.tsx`, `routing/RoutedShell.tsx`, `ReusablePages.gateway.test.tsx` (new).
Docs: this session, `public/frontend/dashboard.md`, the scope status flip, the scope README, `skills/nav/SKILL.md`.
