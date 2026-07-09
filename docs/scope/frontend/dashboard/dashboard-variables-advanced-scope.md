# Dashboard scope — advanced template variables (Grafana-parity depth)

Status: **scope (the ask)**. Stage 3 of [`grafana-conversion-scope.md`](grafana-conversion-scope.md).
Deepens the shipped variable system ([`widget-config-vars-scope.md`](widget-config-vars-scope.md)) and its
frozen `vars` lib (`ui/src/lib/vars/`). Promotes to
[`public/dashboard/dashboard.md`](../../../../doc-site/content/public/dashboard/dashboard.md) on ship.

One paragraph: our variables are real (`query`/`custom`/`text`/`const`/`interval`/`source`, multi,
includeAll, URL-synced, one `{tool,args}` resolver, `required` page params) but **shallower than Grafana's**
in the ways that make a real Grafana dashboard work: an option's **label can differ from its value**,
a variable's query can **reference another variable** (chained/dependent), query results can be **filtered
by regex** (with `(?<text>)`/`(?<value>)` capture groups), options can be **sorted**, a variable can declare
**when it re-resolves** (`refresh`), the "All" selection can carry a **custom value** (`allValue`), and the
interpolation **format-hint set** is wider. We add these as **serde-default additive fields on the existing
`Variable`** and one **dependency-ordered resolution pass** in the variable bar — keeping the **one model,
one resolver** doctrine (no per-type code path) so an extension linking the frozen `vars` lib is not broken.
Zero new host verbs; every resolver stays a caps-gated, token-scoped MCP call.

## Goals

- **Label ≠ value per option.** An option is `{text, value}`, not just a string. Sources: a `label : value`
  syntax in `custom`, and capture groups in a query variable's `regex`. Interpolation substitutes `value`;
  the bar shows `text`.
- **Chained / dependent variables.** A variable whose resolver `args` (or `query`) reference `$otherVar`
  resolve **after** the variables they reference, and **re-resolve** when a dependency's selection changes.
  Derived from parsing `$var` references — no explicit edge in the record (Grafana parity).
- **Regex extraction / filtering** on query-variable results — `regex` + `regexApplyTo:"value"|"text"`,
  including named capture groups for the label/value split.
- **Sort** — `sort` over the option list (alphabetical / numeric, asc / desc, case-insensitive, natural).
- **Refresh mode** — `refresh: "never" | "onLoad" | "onTimeRange"` deciding when options re-resolve
  (distinct from the shipped dashboard-level auto-refresh, which re-runs *panels*).
- **Custom "All" value** — `allValue` (a literal emitted when All is selected, instead of expanding every
  option), and complete **`hide`** modes (`dontHide` / `hideLabel` / `hideVariable`).
- **Wider format hints** — extend `FormatHint` toward Grafana's set (add `regex`, `glob`, `distributed`,
  `percentencode`, `sqlstring`, … the named follow-ups already flagged in `vars/types.ts`).
- **`datasource` variable type** — pick a registered datasource by variable (thin: resolves against the
  federation `datasource.list` under the viewer's caps).

## Non-goals

- **No adhoc filters, no groupby.** Grafana's `adhoc` (key/op/value chips over a datasource) and `groupby`
  variables need datasource key discovery and are datasource-specific — **adhoc is a named follow-up**,
  **groupby is out** (the audit's triage). Stated, not silently dropped.
- **No per-type code path.** Every new capability is a field on the one `Variable` + behavior in the one
  resolver/interpolator — the shipped doctrine ("ALL map to ONE `{tool,args}` resolver or a static form").
  A new *type* is added only where unavoidable (`datasource`), and it still resolves through the one path.
- **No new host verb / no new table.** Variable definitions stay on the `dashboard:{id}` record; resolution
  stays client-issued `{tool,args}` MCP calls.
- **No break to the frozen `vars` lib contract.** Additive only; a `VARS_LIB_V` bump only if `VarScope`'s
  resolved shape must change (it should not — the resolved scope is still `values` + `builtins`).

## Intent / approach

**Deepen the one model, don't fork it.** The `Variable` interface (`ui/src/lib/vars/types.ts`) gains
optional fields; the resolver and the interpolator gain behavior; the record and the frozen resolved
`VarScope` shape are unchanged. Concretely:

- **Options become `{text, value}`.** Add `options?: VariableOption[]` (`{text, value, selected?}`) as the
  resolved/static option list; keep `custom?: string[]` readable (a bare string is `{text:v, value:v}`).
  `custom` strings also parse a `label : value` split. Interpolation already substitutes a value — it now
  substitutes `value` and the bar renders `text`.
- **Chained resolution** is an **ordering pass** in the variable bar, not a new type. A pure helper
  (`vars/depGraph.ts`) extracts `$var` references from each variable's `query.args` / `query.tool` /
  static forms (reusing the shipped `extractVarNamesDeep`), topologically orders them, and the bar resolves
  in that order — interpolating already-resolved variables into a resolver's args before issuing its
  `{tool,args}` call. A **cycle fails honestly** (an error on the offending variable, never a hang).
  A dependency's selection change re-resolves its dependents.
- **Regex / sort / refresh / allValue / hide** are fields consumed at resolve/render time:
  `regex`+`regexApplyTo` filter+capture the resolved rows into options; `sort` orders them; `refresh` gates
  when the bar re-issues the resolver; `allValue` overrides All-expansion in the interpolator; `hide` drives
  the bar's rendering. One small file per concern under `vars/` (FILE-LAYOUT).
- **Format hints** extend the `FormatHint` union + the `formatValue` switch — additive cases, no shape
  change.
- **`datasource` type** resolves its option list from `datasource.list` (federation plane) under the
  viewer's caps — a resolver like any other, just a fixed tool.

**Rejected: a per-type resolver dispatch (a `switch(type)` with a handler each).** That is exactly the
"no per-type code path" the shipped scope forbade; it fractures the one seam extensions link. Every new
capability is a *field* the one resolver/interpolator reads.

**Rejected: an explicit dependency-edge field on `Variable` (`dependsOn: string[]`).** Grafana derives the
graph from `$var` references; storing an explicit edge duplicates a fact the query string already carries
and drifts when the query is edited. We derive it (parse), matching Grafana and the mapper.

## How it fits the core

- **Tenancy / isolation (rule 6):** each variable resolver is a `{tool,args}` MCP call whose workspace comes
  from the viewer's token — a ws-B viewer resolving a shared dashboard's `query` variable gets **ws-B**
  options, never ws-A's. This is the **mandatory isolation test**, extended to a *chained* resolver (the
  dependency's value is interpolated in, but the call is still token-scoped).
- **Capabilities (rule 5/7):** **no new cap.** A `query`/`source`/`datasource` variable resolves under its
  resolver tool's own cap (as today); `datasource.list` is gated by its shipped cap. Deny path: a viewer
  lacking a resolver tool's cap sees that variable's options empty/denied (opaque), not the dashboard broken
  — the mandatory deny test, extended to a variable resolver.
- **Placement (rule 1):** client-side resolution + interpolation; `either`, no role branch.
- **MCP surface (§6.1):** **no new verb.** Variable options resolve via the shipped resolver dispatch /
  `viz.query`; `datasource` options via the shipped `datasource.list`. The API-shape decision is "no new
  API — additive record fields + client resolution".
- **Data (SurrealDB):** `variables[]` on the `dashboard:{id}` record gains optional fields; no new table.
  The per-viewer *selection* still lives in the URL (`?var-`), unchanged. State vs motion holds.
- **Extensions (rule 10) + SDK/WIT:** the `vars` lib is **the** frozen plugin boundary — flagged loudly.
  All additions are optional fields on `Variable` (a *definition* type) and additive `FormatHint` cases; the
  **resolved `VarScope` shape (`values` + `builtins`) is unchanged**, so a linked extension is not broken and
  `VARS_LIB_V` does **not** bump. If chained resolution ever needed the resolved shape to change, that is a
  major bump + a receiver-rejection path — called out as the one place this could break the contract.

## Example flow

1. An author defines `$region` (a `query` variable → `store.query "SELECT name, code FROM region"`), with
   `regex:"(?<text>.+) \\((?<value>[A-Z]+)\\)"` so options show `"West (WST)"` but interpolate `"WST"`.
2. The author defines `$host` (a `query` variable) whose resolver args reference `$region`:
   `{ tool:"store.query", args:{ sql:"SELECT host FROM node WHERE region = '$region'" } }`.
3. On load, `vars/depGraph.ts` parses `$region` out of `$host`'s args → orders `region` before `host`. The
   bar resolves `region` first, the viewer picks `West (WST)`, then `host` resolves with `region='WST'`
   interpolated in — under the viewer's token, so ws-B sees ws-B hosts.
4. The viewer changes `region` to `East`; `host` re-resolves (it depends on `region`), its selection resets,
   dependent panels re-query. A `refresh:"onTimeRange"` variable additionally re-resolves when the time range
   changes.
5. The author sets `$host` `multi:true`, `includeAll:true`, `allValue:".*"`. Selecting All interpolates
   `.*` (the custom value) into panel queries instead of expanding every host — `${host:regex}` still emits a
   regex-safe alternation when All expands.
6. Later this dashboard is imported from Grafana: the mapper maps Grafana's `VariableModel`
   (`regex`/`sort`/`refresh`/`allValue`/`__text`/`__value`/chained queries) onto these fields 1:1
   ([`viz/import-export-scope.md`](viz/import-export-scope.md)); an unsupported `adhoc`/`groupby` degrades
   honestly.

## Testing plan

Per [`../../testing/testing-scope.md`](../../testing/testing-scope.md) — real gateway, real store, seeded
real rows, no `*.fake.ts`.

- **Unit (`vars/*.test.ts`):**
  - `depGraph` — topological order of a `$a`→`$b`→`$c` chain; a **cycle fails honestly** (throws/flags, no
    hang); an independent set keeps insertion order.
  - label/value — a `label : value` custom string and a `(?<text>)/(?<value>)` regex both yield
    `{text≠value}`; interpolation substitutes `value`, the bar label is `text`.
  - regex `regexApplyTo` value vs text; `sort` variants (alpha/numeric/asc/desc/natural); `allValue`
    overrides All-expansion; new `FormatHint` cases (`regex`/`glob`/…) format correctly.
- **Gateway (`features/dashboard/variablesAdvanced.gateway.test.tsx`):**
  - a chained `$region`→`$host` dashboard resolves in order against the **real** store; changing `region`
    re-resolves `host`.
  - **Workspace isolation (mandatory):** the same shared dashboard resolves **ws-B** options for a ws-B
    viewer's `$host`, never ws-A's — including through the chained resolver.
  - **Capability deny (mandatory):** a viewer lacking the resolver tool's cap gets that variable's options
    denied opaquely; the rest of the dashboard still renders.
- **Additivity (mandatory regression):** a pre-advanced-variables dashboard (old `Variable` shape) loads and
  round-trips **byte-clean**; the resolved `VarScope` shape is unchanged (no `VARS_LIB_V` bump).
- **Mapper (referenced, in import-export scope):** a Grafana `VariableModel` with regex/sort/refresh/
  allValue/chained-query maps onto these fields and round-trips; `adhoc`/`groupby` degrade honestly.

## Risks & hard problems

- **The frozen `vars` lib.** Extensions link it; the discipline is *additive fields on the definition,
  unchanged resolved `VarScope`*. Any drift into the resolved shape is a `VARS_LIB_V` major bump — the
  one contract-break risk, and the reason the resolved scope stays `values`+`builtins`.
- **Chained resolution cycles + fan-out.** A cycle must fail honestly, not hang; a deep chain must not
  re-resolve the whole graph on every keystroke (resolve only downstream of the changed variable). The
  dependency pass is pure and unit-tested for both.
- **Regex is user input.** A pathological `regex` can ReDoS the bar; bound it (timeout / length cap) and
  fail the variable honestly rather than freeze the page.
- **All-expansion vs `allValue` vs format hints** interact (`${var:regex}` on an expanded All vs a custom
  `allValue`). Pin the precedence in the interpolator and test the matrix — this is where Grafana users have
  the sharpest muscle memory.
- **Selection persistence on re-resolve.** When a dependency changes and options change, a now-invalid
  selection must reset to a sane default (first option / All), not silently interpolate a stale value.

## Open questions

- **Chained resolution transport** (from the umbrella): confirmed **client-side ordering pass** here (each
  resolver is already a client `{tool,args}` call; no new verb). Revisit only if per-variable round-trips
  are too chatty on a deep graph — the fallback is a batch "resolve dashboard variables" host verb
  (documented, not built).
- **`datasource` variable type:** resolve directly against `datasource.list`, or model it as a `source`
  variable with a fixed resolver tool? Lean a first-class `type:"datasource"` that internally calls
  `datasource.list` (keeps the picker honest); resolve in build.
- **Format-hint completeness:** which of Grafana's ~20 hints ship now vs stay follow-ups? Lean: add the ones
  with clear sinks (`regex`, `glob`, `percentencode`, `sqlstring`, `distributed`); defer the datasource-
  language ones (`lucene`) as named follow-ups.
- **`refresh:"onTimeRange"` wiring:** reuse the shipped time-range change signal the panels already listen
  to? Lean yes (one signal, no new plumbing).

## Related

- [`grafana-conversion-scope.md`](grafana-conversion-scope.md) — the umbrella + the audit ranking these a
  "close now" gap · [`viz/import-export-scope.md`](viz/import-export-scope.md) — the mapper that carries
  Grafana `VariableModel` → these fields.
- [`widget-config-vars-scope.md`](widget-config-vars-scope.md) — the shipped variable system + the frozen
  `vars` lib this deepens · [`reusable-pages-scope.md`](reusable-pages-scope.md) — `Variable.required`, the
  precedent for an additive `Variable` field.
- [`../../datasources/datasources-scope.md`](../../datasources/datasources-scope.md) — the `datasource.list`
  plane the `datasource` variable type resolves against.
- Grafana reference: `/tmp/grafana/packages/grafana-data/src/types/templateVars.ts` (VariableModel),
  `kinds/dashboard/dashboard_kind.cue` (`#VariableModel`/`#VariableOption`/`#VariableSort`/`#VariableRefresh`/
  `#VariableHide`), `public/app/features/templating/formatVariableValue.ts` (format hints),
  `public/app/features/variables/constants.ts` (`$__all`/`allValue`).
- Public: [`../../../../doc-site/content/public/dashboard/dashboard.md`](../../../../doc-site/content/public/dashboard/dashboard.md).
- README **§3** (rules 1/5/6/7/10), **§6.1** (API shape — why no new verb).
