# Frontend dashboard — variable model + bar + URL sync — Slice 2 (session)

- Date: 2026-06-28
- Scope: ../../scope/frontend/dashboard/widget-config-vars-scope.md (Slice 2)
- Status: done
- Public: ../../public/frontend/dashboard.md → "Dashboard variables"
- Tests: rust/crates/host/tests/dashboard_test.rs (`dashboard_variables_round_trip`),
  ui/src/features/routing/search.test.ts (URL round-trip), ui/src/features/dashboard/vars/resolveOptions.test.ts,
  ui/src/features/dashboard/DashboardView.gateway.test.tsx (Slice 2 — define → persist → URL sync → reload)

## Goal

A Grafana-style variable system: define variables on the dashboard record, render a bar of dropdowns,
resolve query options over the bridge, and sync the selected values to the URL (`?var-<name>=`, repeated
for multi). Definitions on the record; selection in the URL (per-viewer, shareable). No new verb.

## What shipped

Backend (additive serde):
- `Dashboard.variables: Vec<Variable>` + the `Variable` struct (one model: name → resolver `{tool,args}`
  for query/source, else static custom/text/const/interval), `#[serde(default)]` throughout — a
  pre-variables dashboard round-trips unchanged. Field renames align the wire with the TS contract
  (`type` via `r#type`, `const` via `const_`, `includeAll` via `include_all`). `dashboard_save` gained a
  `variables` param; the gateway `SaveDashboard` body + the MCP `dashboard.save` dispatcher carry it
  (both additive/defaulted). Exported as `lb_host::DashboardVariable`.

Frontend:
- `ui/src/lib/dashboard/dashboard.types.ts` — `Dashboard.variables?` (re-exports the vars-lib `Variable`).
  `saveDashboard(id,title,cells,variables?)` + the `dashboard_save` IPC case carry `variables` (the
  load-bearing fix — the http case previously dropped it).
- `ui/src/features/routing/search.ts` — `DashboardSearch` gains `refresh?` + flat `var-<name>` params;
  `validateDashboardSearch` parses them (malformed degrades to defaults, never throws);
  `varsFromSearch`/`withVar` translate to/from a bare-name selection map. `REFRESH_OPTIONS` for Slice 4.
- `ui/src/features/dashboard/vars/resolveOptions.ts` — static option lists + `rowsToOptions` (shape a
  tool result into options across the shapes our read tools return).
- `ui/src/features/dashboard/vars/useVariableOptions.ts` — resolve a query/source variable's options over
  the leashed bridge (`makeWidgetBridge([tool])`); a deny/failure is an honest empty list + flag.
- `ui/src/features/dashboard/vars/VariableBar.tsx` — a dropdown per variable (single/multi/include-all),
  a text input for `text`, hidden for `const`; selection writes up via `onChange`.
- `ui/src/features/dashboard/vars/VariableEditor.tsx` — add/edit/reorder variables in a Sheet; query/
  source picks its resolver via the source picker (no tool name typed); `ListField` keeps raw comma-text
  locally so a mid-typed comma isn't eaten by a live split.
- `ui/src/features/dashboard/DashboardView.tsx` — renders the bar + the editor (gated on the edit cap);
  `onSearchChange` (replacing `onRangeChange`) routes range + refresh + var selection through one navigate.
- `ui/src/features/routing/createAppRouter.tsx` — `DashboardsRoute` passes `onSearchChange`.

## Decisions

- **Selection in the URL, definitions on the record** (Grafana parity) — `var-<name>` are flat URL keys
  (round-trip through TanStack) translated by `varsFromSearch`/`withVar`; only the definitions persist.
- **One resolver, no per-type code path** — query/source resolve `{tool,args}` over the same leashed
  bridge a cell uses (host re-checks the cap + workspace); custom/interval are static.
- **Deny/empty is honest** — a denied query variable resolves to an empty option list + a flag, never a
  fabricated catalogue.

## Tests + green output

Backend — `cargo test -p lb-host --test dashboard_test`: **7 passed** (incl. `dashboard_variables_round_trip`
— a query + interval variable round-trip; a save without variables reads back empty).

Frontend unit — `vitest run` (full suite): **103 passed** (search URL round-trip incl. multi-repeat +
malformed-degrades; resolveOptions shaping/dedup/empty).

Frontend real-gateway — `vitest run --config vitest.gateway.config.ts DashboardView.gateway.test.tsx`:
**5 passed** (Slice 2 — open the editor, define a custom multi/include-all variable, save → it persists on
the record; the bar shows the dropdown with real options; selecting a value syncs to the URL search
(`varsFromSearch` → `{env:"prod"}`); a fresh render re-loads the definition and the bar reappears).

## Mandatory categories

- **Capability deny** — a variable query resolves over the leashed bridge; the host re-checks the tool's
  cap + workspace per call (the same machinery as a cell source; deny proven server-side in
  `store_query_test` + `dashboard_test`). A denied query → empty list, never a fake.
- **Workspace isolation** — variable definitions live on the workspace-scoped dashboard record; query
  resolution + selection derive the workspace from the token; the URL var values can't cross the wall
  (the gateway re-derives ws from the token). The shipped two-session dashboard isolation test covers the
  record; URL var values are per-viewer client state.

## Follow-ups

Chained/cascading variables beyond one level, ad-hoc filters, richer multi-value forms — all named
follow-ups, not built. Next: Slice 3 wires interpolation into every cell call + `ctx.vars`/`ctx.timeRange`.
