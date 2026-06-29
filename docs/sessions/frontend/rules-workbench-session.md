# Session — the rules workbench (Playground · chain canvas · datasources admin)

Topic: `frontend` · Scope: [rules-workbench-scope.md](../../scope/frontend/rules-workbench-scope.md) ·
Date: 2026-06-29 · State: **done** · Promotes to
[public/frontend/rules-workbench.md](../../public/frontend/rules-workbench.md)

## The ask

Build the rules workbench frontend from `rules-workbench-scope.md` — all three phases in one session:
the **Playground** (write/run/save a Rhai rule), the **chain canvas** (a React Flow DAG over `chains.*`),
and the **datasources admin page** (over `datasource.*`). The backend is **already shipped**; this slice
is **gateway routes + UI api clients + the React surface** over verbs that exist, mirroring the dashboard
surface verb-for-verb. No host changes, no new MCP verbs, no new caps.

## Key architectural decisions (taken before coding)

- **Gateway routes call `lb_host::call_tool(node, principal, ws, "<verb>", input_json)`** rather than the
  raw host verbs. `call_tool` already (a) re-checks `mcp:<verb>:call` server-side (the gateway's job —
  opaque `Denied`), (b) dispatches the host-native `rules.*`/`chains.*`/`datasource.*`/`federation.*`
  families, and (c) wires the `DisabledModel` AI seam + the `OsLauncher` sidecar for `datasource.test`.
  This is the exact pattern `routes/mcp.rs` uses; doing it any other way would re-implement the model +
  launcher wiring the bridge already owns. ToolError → HTTP: `Denied` → 403 (generic), `BadInput` →
  400 (verbatim — author feedback), `NotFound` → 404. This is the cage/deny honesty mapping the headline
  test asserts.
- **The dev-login `member_caps()` gained the rules/chains/datasource caps.** These caps already exist
  (shipped host verbs); the dev session simply wasn't granted them, so the cap-gated nav + the
  real-gateway frontend tests would have no way to reach the verbs. Granting *existing* caps to the dev
  member is not a new cap and not a host change — it mirrors the dashboard caps already in that list.
  (`rust/role/gateway/src/session/credentials.rs`.)
- **The lead owns all shared shell/routing files** (NavRail, `CoreSurface`, `allowed.ts`, `CAP`,
  `createAppRouter.tsx`, `RoutedShell`, `server.rs`, `routes/mod.rs`) to avoid sub-agent conflicts;
  each sub-agent owns only its isolated feature folder + api client + gateway route file + tests.

## What shipped

All three phases shipped end-to-end in one session (gateway routes + UI api clients + React surface +
tests on both sides), over the SHIPPED `rules.*`/`chains.*`/`datasource.*` host verbs. Decomposed to
three parallel sub-agents (one per vertical slice); the lead reconciled the shared shell/routing files
and the route registrations and ran the full suites.

### Phase 1 — Playground

**Gateway:** `rust/role/gateway/src/routes/rules.rs` — `POST /rules/run`, `GET|POST /rules`,
`GET|DELETE /rules/{id}`. Each `authenticate`s, builds the verb's JSON args, calls
`lb_host::call_tool(...,"rules.<verb>",...)` (server-side cap re-check + `DisabledModel` AI seam) and
maps `ToolError`→HTTP via a `status()` helper: `Denied`→403 opaque "not permitted", `BadInput`→400
**verbatim** (the cage/parse/AI-budget/AI-not-configured author feedback), `NotFound`→404, else→500.

**UI:** `ui/src/lib/rules/{rules.types,rules.api,index}.ts` (one export per verb via `invoke`, the
`RuleOutput` discriminated union on `kind`). `ui/src/features/rules/` — `RulesView`, `RuleEditor`
(CodeMirror `@uiw/react-codemirror` + `lang-javascript`, dirty indicator, Run/Save), `RuleRail`
(list/get/delete), `RunResult` switching `output.kind` → `ScalarCard` | `GridTable` (bounded rows,
"showing N of M") | `FindingsList` (level-coloured, alert-marked) + `LogPanel` + `BudgetBadge` (ms + ai
calls/tokens), `useRules`. The result component renders the typed error **as itself** (the cage/deny
honesty rule) — never a fake result, never a generic toast.

### Phase 2 — chain canvas

**Gateway:** `rust/role/gateway/src/routes/chains.rs` — `GET|POST /chains`, `GET|DELETE /chains/{id}`,
`POST /chains/{id}/run`, `GET /chains/{id}/runs/{run_id}`. Same `call_tool` + `status()` pattern; a
cyclic/invalid DAG arrives as `BadInput`→400 verbatim (the inline edge error source). `save` injects
the workspace from the token, never the body.

**UI:** `ui/src/lib/chains/{chains.types,chains.api,index}.ts`. `ui/src/features/chains/` —
`ChainCanvas` (`@xyflow/react` v12: nodes=steps, edges=needs; Save→`saveChain`, a cyclic edge renders
the host's 400 inline with no crash; Run→`runChain`), `StepNode` (custom node, colour by settle),
`ChainRail`, `chainGraph.ts` (the pure chain⇄{nodes,edges} + snapshot→colour mapping — 1:1 with
`chain.steps[].needs`), `useChainRun` (the **bounded** settle-poll: polls `getChainRun` while the run is
non-terminal, stops on terminal, with a MAX_POLLS ceiling — never an unbounded `setInterval`; a late
open = one snapshot), `useChains`, `ChainsView`. Nodes colour pending/running/ok/err/skipped, the
Halt-pruned subtree greyed, a status banner (success/partialFailure/failed).

### Phase 3 — datasources admin

**Gateway:** `rust/role/gateway/src/routes/datasources.rs` — `GET|POST /datasources`,
`DELETE /datasources/{name}`, `POST /datasources/{name}/test`. Same `call_tool` + `status()` pattern;
`call_tool` wires the `OsLauncher` sidecar for `datasource.test`. The DSN is only in the Add body
(forwarded to the host, never echoed).

**UI:** `ui/src/lib/datasources/{datasource.types,datasource.api,index}.ts` (no `dsn` on any RESPONSE
type — only on the Add request). `ui/src/features/datasources/` — `DatasourcesAdmin` (reuses the shell
`AdminPanel`), `DatasourceRoster` (kind + endpoint + redacted secret ref, NEVER a DSN), `AddDatasourceForm`
(shows the implied `net:tls:host:port:connect` + `secret:federation/{name}:get` grants derived from the
form), `DatasourceProbe` (green/red honest probe), `impliedGrants.ts` (pure), `useDatasources`.

### Shared shell + reconciliation (lead)

- **Route registration:** `routes/mod.rs` (`mod rules|chains|datasources` + `pub use`), `server.rs`
  (the 11 new routes), reconciled across the three sub-agents' additive edits — all merged cleanly,
  gateway builds + tests green.
- **Cap-gated nav:** `NavRail.tsx` (`CoreSurface` += `rules`/`chains`/`datasources` + rail entries),
  `lib/session/admin-caps.ts` (`CAP.rulesRun`/`chainsGet`/`datasourceList`), `routing/allowed.ts`,
  `routing/surface.ts` (`CORE_PATHS`), `routing/createAppRouter.tsx` (imports + wrapper components +
  `coreRoute` entries). Each surface shows on its read cap; the gateway re-checks every verb server-side.
- **Dev-login caps** (`session/credentials.rs`): granted the existing `mcp:rules.*`/`mcp:chains.*`/
  `mcp:datasource.*`/`mcp:federation.query` caps + the defense-in-depth `store:rule:*`/`store:chain:*`
  surface caps the rules/chains verbs re-check below the MCP gate + `secret:federation/*:write` for the
  datasource DSN mediation. These are EXISTING caps granted to the dev member — not new caps, not host
  changes. (Without them the live shell's Playground/canvas/datasources would 403 below the MCP gate.)

## A bug fixed this session (host list verbs)

Building the rail surfaces revealed a **shipped host bug**: `rules.list` and `chains.list` returned an
empty roster even with saved records — they decoded the `lb_store::scan` row directly, but `scan`
returns the `{data, rev}` envelope, so every `from_value` silently failed (and `chains_list` read its
`deleted` tombstone off the envelope too). Fixed both to unwrap the envelope before decoding (mirroring
the working `scan_dashboards`), with roster-contains regression tests in both gateway CRUD tests.
Debug entry: [host/rules-chains-list-drops-every-row-envelope.md](../../debugging/host/rules-chains-list-drops-every-row-envelope.md).
This is the HOW-TO-CODE §3.8 "fix it when building reveals the scope/code was wrong" call — the scope's
CRUD/rail round-trip is impossible with a list verb that always returns empty; the "no host changes"
boundary was about not *building* new backend.

## The headline test — the cage/deny honesty rule (proven green)

The `RulesError`/`ChainsError`/`FederationError` → `ToolError` → HTTP mapping is asserted as itself, not
swallowed into a generic toast:
- a cage error (`eval(...)`) → 400 with the verbatim message (`a_cage_error_is_400_with_the_verbatim_message`);
- an AI rule in a workspace with no model → 400 "AI not configured" (`an_ai_rule_with_no_model_is_400_ai_not_configured`);
- a cyclic DAG at `chains.save` → 400 with the "cycle" validation message (`a_cyclic_dag_is_400_with_the_validation_message`);
- a deny is opaque (`Denied`→403 generic) — proven by the per-verb deny tests;
- the datasource probe with no sidecar → an honest RED, never a fabricated green
  (`test_probe_without_a_sidecar_is_an_honest_red` + the UI `data-state="red"` assertion).

## Green test output

**Rust — full gateway suite** (`cd rust && cargo test -p lb-role-gateway`): every test file green,
including the three new slices:

```
tests/rules_routes_test.rs         test result: ok. 13 passed; 0 failed
  rules_crud_round_trip_over_the_gateway ... ok   (list CONTAINS the saved rule — host bug fixed)
  run_a_scalar_rule_returns_a_scalar_output ... ok
  run_a_grid_rule_returns_a_grid_output ... ok
  run_an_alert_rule_returns_findings_with_log_and_budget ... ok
  a_cage_error_is_400_with_the_verbatim_message ... ok
  an_ai_rule_with_no_model_is_400_ai_not_configured ... ok
  running_a_missing_saved_rule_is_404 ... ok
  {run,save,get,list,delete}_without_the_cap_is_denied ... ok (x5, deny per verb)
  two_sessions_are_workspace_isolated ... ok
tests/chains_routes_test.rs        test result: ok. 5 passed; 0 failed
  chains_crud_round_trip_over_the_gateway ... ok  (roster CONTAINS the saved chain)
  a_cyclic_dag_is_400_with_the_validation_message ... ok
  run_then_runs_get_snapshot_round_trips ... ok   (settle-colouring source)
  each_chains_verb_is_denied_without_its_cap ... ok
  two_sessions_are_workspace_isolated ... ok
tests/datasources_routes_test.rs   test result: ok. 5 passed; 0 failed
  add_then_list_round_trip_over_the_gateway ... ok
  each_verb_is_denied_without_its_cap ... ok
  two_sessions_are_workspace_isolated ... ok
  test_probe_without_a_sidecar_is_an_honest_red ... ok   (DSN-redaction asserted in the list round-trip)
  remove_drops_the_source ... ok
(plus admin/assets/bus/dashboard/data_console/ext_ui/gateway/publish_install all green)
```

**Host crate** (`cargo test -p lb-host`): all green (the `rules`/`chains` list-bug fix verified).

**UI Vitest, real in-process gateway** (`cd ui && pnpm test:gateway`):

```
src/features/rules/RulesView.gateway.test.tsx        (6 tests) ✓
src/features/chains/ChainsView.gateway.test.tsx      (4 tests) ✓
src/features/datasources/DatasourcesAdmin.gateway.test.tsx (5 tests) ✓
  Test Files  3 passed (3)   Tests  15 passed (15)
```

**UI unit** (`cd ui && pnpm test`): `Test Files 18 passed (18) · Tests 114 passed (114)`.
**Typecheck** (`npx tsc --noEmit`): clean.

> Note: the **full** `pnpm test:gateway` run showed one unrelated flake —
> `system/SystemView.gateway.test.tsx`'s "bus peers list" assertion — which **passes in isolation** and
> is the known Zenoh peer-discovery timing flake under concurrent load (see the existing bus-timing
> debug entries). Not touched by this slice.

## Open questions / scope updates

All Phase-1/2/3 decisions in the scope were honored exactly (CodeMirror not Monaco; React Flow v12;
`chains.runs.get` bounded poll, NOT `chains.watch` SSE; three output components by `kind`; DSN
write-only / never read back; federation extension stays headless / the page is first-party shell code;
no new caps / tables / `localStorage` / `if cloud`). The scope's named follow-ups remain follow-ups
(`chains.watch` SSE; per-rule/chain sharing; "explain this deny" affordance; datasource edit). One
addition the scope didn't anticipate: the host `rules.list`/`chains.list` envelope bug had to be fixed
for the rail round-trip to work (above).

