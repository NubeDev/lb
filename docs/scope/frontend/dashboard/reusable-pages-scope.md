# Dashboard scope — reusable pages (template dashboards, instances as bindings, tag-driven fan-out)

Status: scope (the ask). Promotes to `public/frontend/dashboard.md` → "Reusable pages" once shipped.
Builds on the **shipped** variable system ([`widget-config-vars-scope.md`](widget-config-vars-scope.md)),
the **shipped** tags graph (`scope/tags/tags-scope.md`), the **in-flight** nav builder
(`scope/nav/nav-builder-scope.md`), and the **next-up** library panels
([`library-panels-scope.md`](library-panels-scope.md)). Build order: nav → library panels → this.

We want **one dashboard page used many times** — a "Site Overview" authored once and navigable as
Plant-1, Plant-2, Plant-3, … with a new page appearing the moment a new site exists, and zero copies of
the dashboard JSON. Today every piece of the answer exists but nothing joins them: variables re-point a
dashboard (`?var-site=plant-1` is shareable URL state, shipped), tags classify entities
(`tag:['site','plant-1']`, shipped), and the nav scope's `tag-group` dynamically lists *dashboards* by
facet — but there is no way to say "**this** dashboard, once **per** site." The only reuse mechanism for
a parameterized page is hand-authoring N links or duplicating the dashboard N times, and duplication is
exactly the drift problem library panels exist to kill one level down.

---

## The headline idea: instance = binding, never copy

The reuse stack has three altitudes, and each reuses by **reference + parameter**, never by copy:

| Altitude | The reusable thing | How it's reused | Scope |
|---|---|---|---|
| Widget | `panel:{id}` (library panel) | `panel_ref` on a cell + bounded overrides | `library-panels-scope.md` |
| Page | `dashboard:{id}` — its `variables[]` **are** its parameter list | a **binding**: `{ dashboard, var values }` | **this scope** |
| Menu | `nav:{id}` | team share + resolve | `nav-builder-scope.md` |

A **template dashboard** is not a new record type — it is an ordinary dashboard whose variables are
treated as **parameters**. A **page instance** is a *binding* of values onto those parameters, and a
binding has exactly three carriers, from ephemeral to dynamic:

1. **The URL** (shipped) — `?var-site=plant-1` *is* an instance; shareable, per-viewer, durable nowhere.
2. **A nav entry** (small additive change to the nav scope) — a `dashboard` entry carries optional
   pinned `vars`; a curated, durable, named instance ("Plant-1 Overview" in the menu).
3. **A template-group nav entry** (the new piece) — one entry that expands at `nav.resolve` time into
   one instance **per option value** of a variable (tag facets or any `{tool,args}` option source):
   tag a new site → a new page appears in the menu, no nav edit, no dashboard edit.

No new table. No `page_instance` asset. The dashboard record, the variable model, the tags graph, and
the nav resolver already carry everything; this scope adds two additive fields and one nav entry kind.

## Goals

- **`Variable.required: bool`** (additive `#[serde(default)]` on the shipped `Variable`) — marks a
  variable as a **page parameter**. A dashboard opened with a required variable unbound (no URL value,
  no default) renders an honest **"select a \<label\>" empty state** — the variable bar highlighted, cells
  in a waiting placeholder — never a wall of broken/`$site`-literal charts. This is what makes a
  template dashboard *feel* like a template instead of a misconfigured page.
- **Nav `dashboard` entries gain `vars: Record<String, String>`** (additive, optional) — a pinned
  binding rendered into the link as `?var-<name>=<value>`. One dashboard, N curated menu entries.
  *(Flagged to the nav build in flight: keep `items[]` typed so this lands as a field add.)*
- **A `template-group` nav entry kind** — the dynamic fan-out:

  ```
  { kind: "template-group",
    label: "Sites",
    dashboard: "dashboard:site-overview",
    var: "site",                                  // the template's parameter to bind
    options: { facets: [{ key: "site" }] }        // tag-facet source (the common case)
          |  { tool, args }                       // or any option source, the Variable.query shape
  }
  ```

  `nav.resolve` expands it to one child link per option value — label = the value (or the tagged
  entity's title), href = the dashboard with `?var-site=<value>` — reusing the same resolution the
  variable bar already does for a Query variable, under the **caller's** caps, capped like `tag-group`.
- **Binding precedence, stated once:** explicit URL value > nav-entry pinned `vars` > the variable's
  own default > unbound (→ required-empty-state). The nav link *sets* the URL; after that the URL is
  the single source of truth (the shipped model, unchanged).
- **The standalone panel page participates** — `/t/$ws/panel/{id}` (library-panels) already carries
  `?var-` selections; a nav `panel` entry (when that lands) takes the same optional `vars` binding. One
  binding grammar across both page kinds.
- **Template authoring affordance** — the dashboard settings' variable editor shows a "required
  (page parameter)" toggle; the dashboard header shows a small "template — N parameters" hint when any
  variable is required. Nothing heavier: authoring a template *is* authoring a dashboard.

## Non-goals

- **No dashboard duplication / copy-with-sync.** A `dashboard.duplicate` that clones cells is the
  drift machine this scope exists to avoid. (A plain one-shot duplicate as an authoring convenience is
  orthogonal and out of scope here.)
- **No `page_instance` table / third asset.** URL + nav-entry bindings carry every named use we have.
  A durable per-instance record earns its way in only when an instance needs *state of its own* beyond
  var values (per-instance overrides, per-instance sharing) — named deferral, see Open questions.
- **No per-instance spec overrides.** An instance differs from its template **only** by variable
  values. "Plant-1's page needs an extra chart" = a real second dashboard (fork, explicit), exactly the
  library-panels Unlink stance one level up.
- **No new grant path (the lens rule, again).** A binding is data in a menu/URL. The dashboard read
  passes the normal three gates; every cell source re-checks under the **viewer's** caps per call;
  `template-group` expansion runs under the caller's caps (an option value whose backing entity the
  caller can't see is not emitted). Same thesis as nav: showing a link never grants the page.
- **No cross-workspace templates.** The wall holds (rule 6).
- **No `*.fake.ts`.** Real gateway, real seeded dashboards/tags/navs.

## Intent / approach

**Treat `variables[]` as the parameter list it already is.** Grafana's answer to "one dashboard, many
fleets" is template variables + URL — we shipped that. What Grafana lacks (and users hand-roll with
dashboard-list panels) is the *instantiation surface*: something that enumerates the parameter space
and mints one navigable page per value. We put that surface where navigation already lives — the nav
resolver — and drive the enumeration from the same two sources the variable system already resolves
from: **tag facets** (the classification plane; the common case) and **any `{tool,args}` option query**
(the general case). No new resolution machinery: `template-group` expansion IS a Query-variable
resolution, executed server-side in `nav.resolve` where tag-groups already expand.

**Why the nav resolver and not the dashboard?** The dashboard must stay instance-blind — it renders
whatever binding arrives via URL, which keeps *every* carrier (a shared link, a nav entry, a channel
message, a future GenUI reference) equal and keeps the record free of navigation concerns. The nav is
where "what pages exist for me" is answered; fan-out is a navigation question.

**Rejected alternatives:**

- *Copy-on-instantiate (`dashboard.duplicate` per site).* Rejected — N drifting JSON copies; editing
  the template becomes an N-dashboard chore. This is the problem statement, not a solution.
- *A `page_instance` record table.* Rejected for v1 — a third asset whose entire payload is
  `{dashboard, vars}` duplicates what a nav item/URL already says, adds CRUD verbs + caps + share
  semantics for no new capability, and creates a second place instance lists go stale. Revisit only
  when instances need own-state (deferral below).
- *Expanding template-groups client-side in NavRail.* Rejected — `nav.resolve` exists precisely so the
  UI renders one payload with cap-stripping done behind the wall once; client-side expansion would
  re-implement option resolution + cap checks per client (the same reasoning as library-panels'
  host-side hydration).
- *A distinct "template" record type.* Rejected — a template is a dashboard with required variables;
  a flag on `Variable` is the whole difference. A separate type forks the editor, the verbs, the share
  model, and the cache for zero behavior.
- *Binding by tag on the dashboard itself (tag the dashboard `site:plant-1` N times).* Rejected —
  conflates classification of the *record* with parameterization of the *view*; a dashboard tagged
  with N sites is one page, not N pages, and tag-groups already consume dashboard tags for the
  "list dashboards by tag" case. The two mechanisms stay orthogonal: `tag-group` = many dashboards,
  one entry each; `template-group` = one dashboard, many bindings.

## How it fits the core

- **Tenancy / isolation (rule 6):** nothing new crosses anything — bindings reference a
  workspace-local dashboard id; `template-group` expansion resolves tags/tools in the authenticated
  `ws`; a binding naming a ws-A dashboard cannot resolve from ws-B (the nav item is stripped exactly
  like an unreadable `dashboard` entry). Tested (mandatory).
- **Capabilities (rule 5):** **no new caps.** `Variable.required` rides `dashboard.save`
  (`mcp:dashboard.save:call`); nav-entry `vars` + `template-group` ride `nav.save`/`nav.resolve`
  (the nav scope's caps); expansion's option query executes under the caller's existing tool caps —
  a caller without the option source's cap gets the entry stripped (lens), and the dashboard + its
  cell sources re-check server-side on visit regardless. **Deny path:** a `template-group` whose
  `options.tool` the caller lacks → entry stripped, no option values leak (opaque); the direct URL to
  an instance still gates on the dashboard's own read caps.
- **Symmetric nodes (rule 1):** resolver logic in the nav verb + two serde-default fields; no role
  branch.
- **One datastore (rule 2):** zero new tables. `required` on the dashboard record, `vars`/
  `template-group` inside `nav.items[]` (already typed).
- **State vs motion (rule 3):** bindings are state (URL/nav record). Fan-out freshness rides the
  existing "nav re-resolves on visit" posture; no bus event.
- **Core knows no extension (rule 10):** an option source is an opaque `{tool,args}` — a
  `<ext>.<verb>` option query is data, never a branch.
- **MCP surface (§6.1):** **no new verbs.** `dashboard.save`/`get` round-trip `required`;
  `nav.save`/`nav.resolve` round-trip/expand the new item shapes. Batch/live: N/A (nav posture).
- **SDK/WIT impact:** none. `ctx.vars` already delivers resolved values to widgets; a widget on a
  template page is indistinguishable from a widget on any dashboard.
- **One responsibility per file:** the expansion is one resolver file next to the tag-group expansion
  in the nav module (`nav/resolve_template_group.rs` or the TS equivalent seam if resolution is
  split); the required-empty-state is one component (`features/dashboard/vars/RequiredVarGate.tsx`).
- **Skill doc (§6):** extend `skills/nav/SKILL.md` with "publish a template dashboard as a
  per-site menu" (written by the implementing session from a live run).

## Example flow

1. Ada builds `dashboard:site-overview`: a Query variable `site` (options from
   `tags.find {facets:[{key:"site"}]}`), marked **required**; cells read
   `series.read { series: "hvac.${site}.temp" }`, a `store.query` table bound `WHERE site = $site`.
   Several cells are `panel_ref`s to library panels shared across the workspace.
2. Opening the dashboard bare shows the **"select a site"** state — variable bar lit, cells waiting.
   Picking Plant-1 sets `?var-site=plant-1`; she copies the URL into a channel — that link *is* the
   Plant-1 instance.
3. In the nav builder she adds one **template-group** to `nav:ops`:
   `{ label: "Sites", dashboard: "dashboard:site-overview", var: "site", options: { facets: [{ key: "site" }] } }`.
4. Ben (team ops) resolves the nav: **Sites ▸ Plant-1 · Plant-2 · Plant-3** — three menu pages, one
   dashboard record. Each link lands on `?var-site=…`; `${__user.login}` is Ben's; every cell source
   re-checks under Ben's caps.
5. A new cooler site is commissioned and its entities get tagged `site:plant-4`. Ben's next visit
   shows **Plant-4** — nobody edited a nav or a dashboard.
6. The template needs a new gauge: Ada edits `dashboard:site-overview` once; all N instances show it —
   and because the gauge is a library panel, the *exec* dashboard referencing the same panel updates
   too. Edit-once at both altitudes.
7. Plant-2 needs a special one-off chart: that's a **fork** — Ada duplicates into a real
   `dashboard:plant-2-custom` and pins it as a plain `dashboard` nav entry with
   `vars: { site: "plant-2" }`. Drift is explicit and hers.

## Testing plan

Per `scope/testing/testing-scope.md` — real store/caps/gateway/tags, seeded records, no mocks:

- **Capability deny (mandatory):** `template-group` whose option tool the caller lacks → entry
  stripped in `nav.resolve`, option values not enumerable (opaque); the instance URL still denied
  server-side without the dashboard/source caps.
- **Workspace isolation (mandatory):** ws-B resolving a nav with a ws-A dashboard binding gets it
  stripped; ws-B's `template-group` expansion sees only ws-B tags; two real sessions.
- **The lens (headline):** a template-group emitting N instance links never widens access — a viewer
  lacking a series cap sees the menu entry (if they can read the dashboard) but the cell data denies
  per call, same as inline.
- **Expansion:** tag `site:plant-4` → re-resolve → new entry; untag → gone; the 50-per-group cap
  truncates loudly (the tag-group rule); label falls back sanely for typed (non-string) facet values.
- **Binding precedence:** URL beats nav-pinned `vars` beats variable default; unbound required var →
  the empty state, cells do not fire (no `$site`-literal queries hit the gateway — assert zero calls).
- **Round-trip:** `Variable.required` and nav `vars`/`template-group` survive save→get; old records
  without the fields load unchanged (serde default).
- **UI (`pnpm test:gateway`):** author the template + template-group through the real builder →
  resolve → click an instance → the page renders bound; the bare template shows the required gate.

## Risks & hard problems

- **Required-var UX is the make-or-break.** If an unbound template renders broken charts (literal
  `$site` in queries, empty errors), templates feel like a footgun. The gate must hold cell firing
  *before* any bridge call, not mask errors after.
- **Expansion cost + staleness.** A template-group is one more `tags.find`/tool call inside
  `nav.resolve` — bounded by the same 50-cap and the visit-scoped resolve posture. Do not add a live
  nav push for this; re-resolve-on-visit is the stated freshness.
- **Option-value hygiene.** Values become URL params and interpolation inputs; they are already
  treated as data everywhere (bound `vars`, JSON tree substitution — never spliced), but the expansion
  must URL-encode and must not let a weird tag value break the link grammar.
- **Two dynamism mechanisms, one mental model.** `tag-group` (many dashboards) vs `template-group`
  (one dashboard, many values) will be confused in the builder UI; name and describe them side by side
  in the picker ("Dashboards by tag" / "One dashboard per ⟨value⟩").
- **Scope creep toward per-instance overrides.** The moment someone asks for "just hide one cell on
  Plant-3," hold the line: fork (a real dashboard) or nothing. A patch layer here recreates the
  library-panels tar pit at page altitude.

## Decisions (proposed; confirm at build start)

- **Instance carriers: URL + nav entry only; no `page_instance` table** — revisit only when an
  instance needs own-state (per-instance share/annotations). Named deferral.
- **Fan-out lives in `nav.resolve`, server-side** — one expansion seam behind the wall (the
  library-panels hydration precedent).
- **A template is a dashboard + `required` variables** — no new record type, no new verbs, no new caps.
- **One `var` per template-group in v1** — multi-parameter fan-out (site × env matrices) is a named
  follow-up; one level covers the fleet case that motivates this.
- **Nav build hook (do now, while nav is in flight):** keep `items[]` typed such that `dashboard`
  entries can gain `vars` additively, and entry kinds are an open enum — the rest of this scope then
  lands without touching the nav schema.

## Open questions

- Per-instance own-state (the `page_instance` deferral) — only if a real ask lands.
- Multi-parameter template-groups (nested fan-out) — follow-up.
- A `panel` entry kind with the same `vars` binding once library panels ship — expected, additive.

## Related

- [`widget-config-vars-scope.md`](widget-config-vars-scope.md) — the shipped parameter substrate:
  `Variable`, `?var-` URL grammar, `ctx.vars`, interpolation; this scope adds `required` to it.
- [`library-panels-scope.md`](library-panels-scope.md) — reuse one altitude down (widget); the same
  reference-not-copy + explicit-fork doctrine, applied to pages here.
- `../../nav/nav-builder-scope.md` — the carrier of durable/dynamic bindings; this scope adds the
  `vars` field and the `template-group` entry kind to its `items[]`.
- `../../tags/tags-scope.md` — `tags.find` facets, the default option source for fan-out.
- `../dashboard-scope.md` — the asset model; `viz/panel-model-scope.md` — the v3 cell the template's
  cells use.
- README §3 (rules 5, 6, 10), §6.1, §6.5.
