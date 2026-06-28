# Frontend dashboard — interpolation wired into every cell call + ctx.vars/timeRange — Slice 3 (session)

- Date: 2026-06-28
- Scope: ../../scope/frontend/dashboard/widget-config-vars-scope.md (Slice 3)
- Status: done
- Public: ../../public/frontend/dashboard.md → "Variable interpolation into cells (+ ctx.vars)"
- Tests: ui/src/features/dashboard/vars/useVarScope.test.ts (scope resolution),
  ui/src/features/dashboard/DashboardView.gateway.test.tsx (Slice 3 — `${host}` → selected series renders)

## Goal

The payoff: a cell re-points by variable. Before any cell `bridge.call(source.tool, source.args)` and
control action call, interpolate the args against the resolved `VarScope`; and hand an extension tile
`ctx.vars` + `ctx.timeRange` (the additive v2 ctx field). The shell resolves the scope from the token +
URL; cells/iframes never resolve identity or query vars themselves.

## What shipped

- `ui/src/features/dashboard/vars/useVarScope.ts` — resolves the `VarScope` shell-side: `values` from the
  URL selection (falling back to const/text/interval defaults), `builtins` from the verified session
  (`${__user.login}`/`${__workspace}`) + the URL time range (`$__from`/`$__to`/`$__range*`) +
  `${__dashboard}` + `$__interval`. Un-spoofable: identity comes from `getSession()` (the token), never a
  cell.
- `ui/src/features/dashboard/builder/useSource.ts` — interpolates `source.args` against the scope BEFORE
  the bridge call (and the watch args); re-keys so a selection/refresh change re-runs the source. For a
  `store.query` source, interpolation runs over the arg tree (the bound `vars`) — never string-spliced
  SQL; the host parse-allowlist stays the boundary.
- The read views (`ChartView`/`StatView`/`GaugeView`/`TableView`) + controls (`Switch`/`Slider`/`Button`)
  take a `scope` prop; controls call `interpolateArgs(action.argsTemplate, scope, runtimeValue)` (the
  `fillArgs` generalization — vars in the template like `${__workspace}` resolve, the `{{value}}` slot
  takes the interaction value, type-preserved).
- `ui/src/features/dashboard/builder/ExtWidget.tsx` — the v2 ctx gains `vars` (resolved selections),
  `builtins`, and `timeRange` (from/to ms), marked `v:2` (`WIDGET_CTX_V`). A v1 widget that ignores them
  is unaffected; the tile re-mounts when the scope changes. The extension NEVER resolves identity itself.
- Threaded `scope` DashboardView (`useVarScope`) → Grid → WidgetHost → WidgetView → views/ExtWidget.

## Decisions

- **One interpolation choke point per call path.** `useSource` for reads, `interpolateArgs` in each
  control — both run the shared lib. No view re-implements substitution.
- **Type-preserving args.** A sole `${var}` arg leaf becomes the raw value (a multi-value → a real array
  for a JSON/IN sink); embedded refs (`cpu.${host}`) string-interpolate. The host re-checks the cap +
  workspace on the interpolated call regardless.
- **ctx is additive + versioned.** `vars`/`timeRange`/`builtins` ride a `v:2` ctx; the bridge call/watch
  signature is unchanged (the frozen v2 widget contract holds).

## Tests + green output

Unit — `vitest run` (full): **106 passed** (incl. `useVarScope`: URL selection → values + token/range
built-ins; const/text/interval defaults; an unselected query var left out so `interpolate` keeps it
literal).

Real-gateway — `DashboardView.gateway.test.tsx`: **6 passed**. The Slice-3 case seeds (real write path) a
dashboard with a `host` custom variable and a chart cell whose source is `series.read {series:"${host}"}`;
rendering with `?var-host=cooler.temp` makes the chart resolve `${host}` → `cooler.temp` and render real
rows read through the bridge — the cell re-points by variable, end to end.

## Mandatory categories

- **Identity un-spoofable:** `${__user.*}`/`${__workspace}` are resolved by `useVarScope` from
  `getSession()` (the verified token) shell-side and passed in as resolved values; a cell/iframe cannot
  set them (the `useVarScope.test` asserts the built-ins come from the session, and the ctx the tile gets
  carries only resolved values — no token).
- **Capability deny / workspace isolation:** interpolation changes only the *args* of a call; the call
  still goes through the leashed bridge + the host re-check (deny + isolation proven server-side in
  `store_query_test`/`dashboard_test` and the shipped widget bridge tests). A `store.query` source binds
  vars, never splices SQL (the parse-allowlist is untouched).

## Follow-ups

Query-variable default resolution ("first option" without a selection) is left to the bar; chained
variables are a named follow-up. Next: the bus.publish/bus.watch platform fix, then Slices 4–5.
