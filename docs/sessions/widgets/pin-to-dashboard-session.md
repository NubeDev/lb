# Widgets — Slice B: pin a tool result-render to a dashboard (session)

- Date: 2026-07-04
- Scope: ../../scope/widgets/pin-to-dashboard-scope.md
- Stage: S9+ (the system-wide widget program, Slice B of `widget-platform-scope.md`)
- Status: done

## Goal

Close G2 of the widget umbrella: a GENERIC path that takes any `x-lb-render` envelope (a tool's
`ToolDescriptor.result`, or a live channel `rich_result` body) and mints a persisted `dashboard:{id}` cell.
The keystone for "widgets are system-wide" — `reminder.list` becomes dashboard-addable with ZERO
reminder-specific code in the pin path. Stage exit gate (MET): the headline pin→reload→render loop works
end to end against a real gateway, the pin is generic over the tool id (rule 10), and Slice A's
save-validator still fires through the pin path.

## What changed

### Backend (Rust)

- **`rust/crates/host/src/dashboard/pin.rs`** (NEW) — `dashboard_pin` + `mint_cell_from_envelope` + the
  `slug` helper + `pin_descriptor`. The mint function is the core work: it mirrors
  `ResponseView.buildCell` (the shipped channel adapter) field-for-field so a pinned cell renders
  identically to the channel response (the cross-surface fidelity invariant), but host-side and emitting a
  v3 cell. `i = pin-{slug(envelope.source.tool || view)}` by pure string ops — no branch on a tool id
  (rule 10). Idempotent: re-pinning the same envelope REPLACES the cell (keeps its layout via the
  `existing: Option<&Cell>` arg); a different envelope APPENDS at `next_free_y`. Reuses the Slice A
  validation chain (`check_cells_bounds` → `check_genui_cells` → `check_view_cells` →
  `validate_and_strip_refs`) → `write_dashboard` → `hydrate_cells` (returns a hydrated record, mirrors
  `dashboard.save`). Gated `mcp:dashboard.pin:call` (its own cap, distinct from `dashboard.save`);
  owner-only-update on an existing dashboard. Unit tests in-file (mint mirrors reminder.list; generic over
  an arbitrary tool id; re-pin preserves layout; malformed envelopes rejected; slug is pure string ops;
  `next_free_y`).
- **`rust/crates/host/src/dashboard/mod.rs`** — `mod pin;` + re-exports (`dashboard_pin`,
  `mint_cell_from_envelope`, `pin_descriptor`).
- **`rust/crates/host/src/dashboard/tool.rs`** — the `dashboard.pin` dispatch arm in `call_dashboard_tool`
  (the headless `POST /mcp/call` path). `envelope` is opaque; `dashboard` + `title?` + `now`.
- **`rust/crates/host/src/lib.rs`** — re-exports `dashboard_pin`, `mint_cell_from_envelope`,
  `pin_descriptor`.
- **`rust/crates/host/src/tools/descriptor.rs`** — `pin_descriptor()` added to `host_descriptors()` so
  `tools.catalog` lists it (an AI discovers it can pin).
- **`rust/role/gateway/src/session/credentials.rs`** — `mcp:dashboard.pin:call` added to `member_caps()`
  (the `.pin` wildcard trap, same as `.catalog`); the `dev_login_carries_the_widget_catalog_read` test now
  also asserts the pin cap.
- **`rust/role/gateway/src/routes/dashboard.rs`** — `POST /dashboards/{id}/pin` route (`pin_dashboards`)
  + `PinDashboard { envelope, title }` body. Uses `gw.now()` (the gateway clock) so the REST client
  doesn't pass `now`.
- **`rust/role/gateway/src/routes/mod.rs`** + **`server.rs`** — route export + `.route("/dashboards/{id}/pin", post(pin_dashboards))`.

### Frontend (UI)

- **`ui/src/features/dashboard/views/table/RowControls.tsx`** (NEW) — the shared actions-column
  renderer, extracted from `ResponseTable`. Used by BOTH the dashboard `TablePanel` (a pinned cell) and
  the channel `ResponseTable` (a live response) so a pinned reminder widget is fully interactive on the
  dashboard — the cross-surface fidelity invariant. One `RowControl` type (re-exported by `ResponseTable`
  for existing callers).
- **`ui/src/features/dashboard/views/table/TablePanel.tsx`** — renders an actions column when
  `options.rowControls` is present (a pinned cell carries it), via the shared `<RowControls>`. The
  `useMemo` for `cellTools` is hoisted above the early returns (hooks order). Absent → read-only table
  (the shipped behavior).
- **`ui/src/features/channel/ResponseTable.tsx`** — refactored to reuse the shared `<RowControls>` (the
  channel adapter keeps its simpler chrome; the actions column is now ONE renderer). Re-exports
  `RowControl` for existing callers.
- **`ui/src/features/channel/PinToDashboard.tsx`** (NEW) — the "Pin to dashboard" affordance mounted by
  `ResponseView` beside a rendered `rich_result`. Picks a target dashboard (from `dashboard.list` + a
  "New dashboard" option) and calls `pinDashboard` over the real gateway. The client passes the ENVELOPE
  through; the host constructs the CELL (no cell construction in the client). Shows a "pinned to <name>"
  confirmation; surfaces a non-owner/cap deny as a short message.
- **`ui/src/features/channel/ResponseView.tsx`** — mounts `<PinToDashboard>` beside the rendered widget.
- **`ui/src/lib/dashboard/dashboard.api.ts`** — `pinDashboard(id, envelope, title?)` client.
- **`ui/src/lib/ipc/http.ts`** — `dashboard_pin` → `POST /dashboards/{id}/pin`.

### Tests

- **`rust/crates/host/tests/widget_pin_test.rs`** (NEW, 10 tests, all green) — capability deny + plain-
  member happy path; non-owner deny; workspace isolation; the HEADLINE (pin reminder.list → reload → cell
  intact); generic-over-tool-id; idempotent re-pin replaces; different envelope appends + re-pin preserves
  layout; shell-vs-headless parity (the SAME cell from `dashboard_pin` and `call_tool` → `dashboard.pin`);
  Slice A view-validator fires through the pin path (`view:"heatmap"` rejected); pin coexists with
  hand-authored cells.
- **`ui/src/features/channel/PinToDashboard.gateway.test.tsx`** (NEW, 4 tests, all green, real spawned
  gateway) — the HEADLINE (pin a reminder.list rich_result via the UI affordance → reload via
  `dashboard.get` → render the cell through the real `WidgetView`/`TablePanel` → reminder rows AND row
  controls visible); capability-deny (a session without `mcp:dashboard.pin:call` refused at the host);
  workspace isolation (ws-B can't see ws-A's pinned dashboard); fidelity + idempotency (re-pin replaces).

## Decisions & alternatives

- **`dashboard.pin` server-side mint verb, NOT client-compose.** The umbrella leaned client-compose
  ("unless a server-side mint proves necessary"); this slice picked the verb. The proof of necessity is
  the same argument Slice A used to put save-validation server-side: a pin produces PERSISTED state, and a
  headless `POST /mcp/call` agent (no shell, no `ResponseView.buildCell`) must be able to pin a tool's
  `result` envelope. With client-compose, every client re-implements the envelope→cell mapping; the host
  can't enforce fidelity. The channel render path (`ResponseView.buildCell`) is UNTOUCHED — it keeps doing
  ephemeral envelope→cell for render; `dashboard.pin` is the persist-time twin, host-side. Rejected:
  client-compose (the host can't enforce the mapping for a headless writer; the envelope↔cell fidelity
  risk is best owned by ONE host function). See the scope doc's "Open question, resolved" for the full
  reasoning.
- **Row controls shared, not duplicated.** The user chose "ship row controls now" — so a pinned reminder
  cell is fully interactive on the dashboard (enable switch, run-now, delete), not read-only. Extracted
  `<RowControls>` (the actions column) shared by the dashboard `TablePanel` and the channel
  `ResponseTable`. Rejected: (a) make `TablePanel` route through `ResponseTable` (different chrome — the
  channel table has no header/sort/formatting); (b) leave row controls channel-only and ship the pinned
  cell read-only (a named follow-up) — rejected because the user asked for the long-term-right call.
- **Idempotency by tool id (`pin-{slug(source.tool)}`), not envelope hash.** One cell per tool per
  dashboard (re-pinning `reminder.list` refreshes the cell, not duplicates). Simpler mental model ("the
  reminder widget is on the dashboard"). Rejected: full envelope hash (one cell per unique envelope) —
  overkill for v1; a named follow-up if a second filter matters. Documented as a known limit in the scope.
- **`dashboard.pin` is its own cap (`mcp:dashboard.pin:call`), distinct from `dashboard.save`.** A member
  who can pin but not free-edit cells still works. The pin reuses the Slice A validation PRIMITIVES (not
  the `dashboard_save` function), so it has its own gate + its own file (FILE-LAYOUT). Rejected: route
  through `dashboard_save` (would require the caller hold BOTH caps — surprising; and the mint is a
  distinct write path, not a cells-array save).

## Tests

Real gateway + real store, no fakes (rule 9). Mandatory categories:

- **Capability deny:** `pin_denied_without_cap_and_allowed_for_a_plain_member` (Rust) — a principal with
  NO caps is opaque-denied; a PLAIN member with only the pin + read caps pins (proves the grant, not an
  admin bypass). `pin_is_denied_for_a_non_owner_on_an_existing_dashboard` (Rust) — owner-only-update. UI:
  `capability-deny: a session without mcp:dashboard.pin:call is refused by the host`.
- **Workspace isolation:** `pin_in_ws_a_is_invisible_to_ws_b` (Rust) + UI `workspace isolation: a pin in
  ws-A is invisible to ws-B`.
- **Envelope↔cell fidelity + idempotency:** `pin_reminder_list_envelope_persists_and_reloads_intact`,
  `re_pin_same_envelope_replaces_in_place_not_duplicates`,
  `pin_a_different_envelope_appends_and_re_pin_preserves_layout` (Rust) + UI
  `fidelity + idempotency: re-pinning the same envelope replaces the cell, not duplicates`.
- **Shell-vs-headless parity:** `shell_path_and_headless_mcp_call_produce_the_same_cell` (Rust) — the SAME
  cell from `dashboard_pin` and `call_tool` → `dashboard.pin`.
- **Slice A view-validator still fires:** `a_hallucinated_view_in_the_envelope_is_rejected_through_pin`
  (Rust) — `view:"heatmap"` rejected through the pin path.
- **The HEADLINE (generic over tool id + renders through WidgetView):** Rust
  `pin_reminder_list_envelope_persists_and_reloads_intact` + `pin_path_is_generic_over_an_arbitrary_tool_id`
  (an arbitrary `__test__.frobnicate` tool id mints a valid cell); UI
  `HEADLINE: pins a reminder.list rich_result to a dashboard, reloads, and renders the rows + row controls
  through WidgetView`.

### Green output (Rust — `cargo test -p lb-host --test widget_pin_test`)

```
running 10 tests
test shell_path_and_headless_mcp_call_produce_the_same_cell ... ok
test pin_is_denied_for_a_non_owner_on_an_existing_dashboard ... ok
test pin_reminder_list_envelope_persists_and_reloads_intact ... ok
test pin_path_is_generic_over_an_arbitrary_tool_id ... ok
test pin_in_ws_a_is_invisible_to_ws_b ... ok
test pin_a_different_envelope_appends_and_re_pin_preserves_layout ... ok
test pin_denied_without_cap_and_allowed_for_a_plain_member ... ok
test a_hallucinated_view_in_the_envelope_is_rejected_through_pin ... ok
test pin_appends_alongside_hand_authored_cells ... ok
test re_pin_same_envelope_replaces_in_place_not_duplicates ... ok

test result: ok. 10 passed; 0 failed
```

Plus Slice A suites still green (`widget_catalog_test` 8/8, `dashboard_test`, `dashboard_genui_test`,
`catalog_mcp_test`) and the credentials test (`dev_login_carries_the_widget_catalog_read` now also asserts
the pin cap). `cargo build --workspace` + `cargo fmt` clean.

### Green output (UI — `pnpm test:gateway src/features/channel/PinToDashboard.gateway.test.tsx`)

```
 ✓ src/features/channel/PinToDashboard.gateway.test.tsx (4 tests) 377ms

 Test Files  1 passed (1)
      Tests  4 passed (4)
```

Plus `pnpm test` (unit) **547/547** green (incl. the radius-scale style guard after fixing a bare
`rounded`); the reminders-palette gateway (uses `ResponseTable`/`RowControls`) **11/11** + DashboardView
gateway (uses `TablePanel`/`WidgetView`) **11/11** — no regression from the shared `RowControls`
extraction or `TablePanel` row-controls.

## Debugging

- **`panel_test` "STALE" — a PRE-EXISTING Slice A blast-radius (NOT this slice's regression).** Surfaced
  while running the broader suite: 4 `panel_test` cases fail with `cell c1/c2: unknown view 'STAKE' — call
  dashboard.catalog for the palette`. The `panel_test` fixtures use `view:"STALE"` as a placeholder echoed
  spec to prove hydration overwrites it; Slice A's `check_view_cells` (in `save.rs:48`, UNCHANGED by this
  slice) rejects "STALE" before ref-stripping runs. `git diff --stat HEAD` confirms this slice's Rust
  changes are purely ADDITIVE (new `pin.rs` + new dispatch arm + re-exports); the validation path in
  `dashboard.save` is byte-identical to before. `git log -- rust/crates/host/tests/panel_test.rs` shows it
  was last touched at `de9e9a7 "added backend widgets"` (the Slice A commit) — so Slice A shipped these
  reds. Logged at `debugging/widgets/panel-test-stale-view-preexisting-slice-a-red.md` (a noted
  follow-up for the Slice A owner / a fixtures update — `STALE` → a real-but-different view like `gauge`
  so the validator passes AND the hydration-overwrite intent is preserved). This slice did NOT absorb the
  fix (out of scope; fixing it risks masking the test's intent without understanding Slice A's design).
- One in-session fix: `TablePanel`'s `useMemo(cellTools)` was initially placed after the early returns →
  "Rendered more hooks than during the previous render." Hoisted above the early returns (hooks must run
  unconditionally). Caught by the gateway test run before claiming done; no regression escaped.

## Public / scope updates

- **Scope:** `scope/widgets/pin-to-dashboard-scope.md` (NEW — written + reviewed before building). The
  umbrella `widget-platform-scope.md` open question "Where does 'pin' live?" is RESOLVED (a `dashboard.pin`
  verb, with the recorded justification); the umbrella's Slice B section is now build-ready.
- **Public:** `public/frontend/dashboard.md` gains a "Pin to dashboard (Slice B)" section.
- **STATUS.md:** the Slice B row added (building → shipped).

## Skill docs

- **`skills/dashboard-widgets/SKILL.md`** updated with a new section "Pin a tool result to a dashboard"
  (the `dashboard.pin` call, the envelope shape, the idempotent re-pin), grounded in the live gateway run
  that the headline test drove. The surface this slice adds is agent-drivable (`dashboard.pin` MCP verb +
  `POST /dashboards/{id}/pin` route), so the skill is the operating manual an agent follows to pin.

## Dead ends / surprises

- The `slug` function initially produced a leading dash for tool ids starting with `_`/`.` (e.g.
  `__test__.x` → `-test-x`), which doubled up with the `pin-` prefix (`pin--test-x`). Fixed by trimming
  both ends (`trim_matches('-')`). Caught by the Rust unit test `slug_is_pure_string_ops` + the
  integration test `pin_path_is_generic_over_an_arbitrary_tool_id`.
- The REST route (`POST /dashboards/{id}/pin`) uses `gw.now()` (gateway clock), but the headless MCP path
  (`call_tool` → `dashboard.pin`) requires `now` in args (the caller's logical clock — determinism §3, the
  same convention `dashboard.save` uses). The UI affordance goes through the REST route (no `now`
  needed); a headless agent over `POST /mcp/call` must pass `now`. Documented in the test.
- The `mcp_call` cap gate runs BEFORE the `now` parse in `call_tool`, so the cap-deny test doesn't need
  `now` (denied at the gate first). Noted in the test.

## Follow-ups

- **`panel_test` "STALE" fixtures** (pre-existing Slice A red) — `debugging/widgets/panel-test-stale-view-preexisting-slice-a-red.md`.
  A Slice A follow-up: change the echoed-spec placeholder from `view:"STALE"` to a real-but-different view
  (e.g. `gauge`) so `check_view_cells` passes AND the hydration-overwrite intent is preserved.
- **Idempotency by envelope hash** (a known limit) — `i = pin-{slug(source.tool)}` means two DIFFERENT
  envelopes from the same tool collide on the same cell. Fine for v1 ("the reminder widget is on the
  dashboard"); widen to an envelope hash if a second filter per tool matters.
- **Slice C** (result-render coverage — give the remaining tools a `descriptor.result` envelope) and
  **Slice D** (channel-origin AI authoring — response → widget → preview → `dashboard.pin`) now build on
  this verb.
- **Per-widget version stamping** (Slice A follow-up) still deferred.
- STATUS.md updated: Slice B shipped.