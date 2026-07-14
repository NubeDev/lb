# Viz Grafana-parity backend — P1 (model fields + the `queryOptions` hole) — session

- Date: 2026-07-14
- Scope: `docs/scope/viz/grafana-parity-backend-scope.md`, Phase **P1**
- Area: `rust/crates/host/src/dashboard/` (model + save path), `rust/crates/host/src/viz/`
  (time override), `rust/role/gateway/src/routes/dashboard.rs` (timezone passthrough)
- Status: **done** — P1 shipped green; P2 (lb-viz tranche 2) and P3 (the import pin) next

## The opener: the `queryOptions` drop — verified, then fixed

Per the scope's risk note, verified **before** fixing, on the real path (no mocks): a
`call_dashboard_tool("dashboard.save", …)` over a mem:// store with a UI-shaped v3 cell carrying
top-level `queryOptions {maxDataPoints, minInterval, relativeTime, …}`, then `dashboard.get`. Run
against pre-fix code the test fails with `queryOptions == Null` on both the save's own return and
the read-back.

**Confirmed honestly: the field was silently dropped, and every shipped save carrying it has lost
that user data — unrecoverable, since it never reached the store.** The drop happens at the MCP
boundary (`dashboard/tool.rs::typed_arg::<Vec<Cell>>`): `Cell` is a closed serde struct with no
catch-all, so unknown top-level cell keys vanish before validation or the store see them. Full
entry: `docs/debugging/dashboard/query-options-silently-dropped-on-save.md` (+ README row).

This also pins the bound on the UI scope's carry-don't-strip guarantee: it holds **inside**
`options`/`fieldConfig`/`custom` (opaque `Value`s), NOT for unknown top-level cell fields.

## What shipped

All additive, serde-defaulted, null-tolerant (`null_default`), camelCase-renamed like
`fieldConfig`. No change to `Cell.v` or `SCHEMA_VERSION`.

- **`QueryOptions` on `Cell`** (`queryOptions`, skip-if-empty so pre-P1 records stay byte-stable):
  the shipped UI trio `maxDataPoints`/`minInterval`/`relativeTime` + Grafana's
  `timeFrom`/`timeShift`/`hideTimeOverride`. Typed (not opaque) because `viz.query` interprets the
  override; the rest ride to the client.
- **`Cell.transparent`** (skip-if-false) and **`Cell.links`** (opaque `Vec<Value>`, skip-if-empty)
  — the host carries them; renderers are the UI scope's problem.
- **`Dashboard.timezone`** — record-carries-the-import, prefs-wins-at-render (this resolves the
  scope's open question: canonical-in/localized-out, matching the doctrine; rejected
  "prefs-only" because an import would then silently lose the dashboard's declared tz).
  Preserve-on-omit through `dashboard_save_meta` exactly like `description`/`icon`/`color`, plus
  the gateway `POST /dashboards` body and the `dashboard.save` descriptor.
- **`Variable.description` / `skipUrlSync` / `allowCustomValue`** — host-opaque definition data.
- **`viz.query` applies the time override** at target dispatch (`viz/time_override.rs`, one
  responsibility per file; wired in `query.rs::dispatch_target`).

## The `timeFrom`/`timeShift` semantics pin (the scope's risk note)

Pinned from Grafana's `applyPanelTimeOverrides` before implementing:

1. `timeFrom` **replaces** the range: effective range = `[now − timeFrom, now]`. An override, not
   a nudge — it wins over the dashboard/caller range (so it deliberately rewrites a
   caller-supplied `from`/`to`; that IS the feature).
2. `timeShift` then moves **both** ends earlier by the shift (`from −= shift`, `to −= shift`).
   Order: timeFrom first, then timeShift — they compose.
3. `hideTimeOverride` is display-only; it never touches the query.

Duration grammar: Grafana's fixed-amount `rangeUtil` math (`s/m/h/d/w`, `M` = 30 d, `y` = 365 d) —
not calendar arithmetic.

**Deliberately bounded (the honest split):** the host applies the override only to a target's
**numeric epoch-second** `from`/`to` args — the `series.read` contract, the one range vocabulary
the platform owns. A non-numeric `from`/`to` (an ext tool's string expression) is left untouched
(the host never guesses another tool's vocabulary — rule 10: the id/args stay opaque data), a
target with no range and no `timeFrom` gets nothing invented, and an unparsable duration degrades
to a no-op, never a failed panel. Grafana's `now-1h/d`-style snapped expressions and calendar
months are **out of P1** — carried on the record verbatim, applied only when they parse. If a
fixture demands the snapped grammar, that's a named P2+ follow-up, not a silent stretch.

## Also noted (follow-ups, honest)

- **`PanelSpec` (library panels) does not carry the P1 fields yet** — a ref cell's hydration
  defaults them and the save-time strip clears them (they are spec-ish, not per-placement
  overrides). Extending `PanelSpec` is a small additive follow-up when the UI edits them on a
  library panel; noted in `panel/hydrate.rs`.
- `maxDataPoints`/`minInterval` are **carried, not enforced** in P1 — enforcement (bucket math at
  dispatch) needs the datasource-interval story and belongs with the import mapper (P3) or a
  dedicated slice.
- Release as a `node-v*` tag so rubix-ai can bump its pin (the scope's P1 exit); tagging is the
  user's call (no commits from this session by instruction).

## Tests (all green, real store / real node, no mocks)

- `crates/host/tests/dashboard_query_options_test.rs` — **the headline regression pin** (fails on
  pre-fix code): UI-shaped `queryOptions` survives the real `dashboard.save` → `dashboard.get`;
  the other P1 fields ride the same path; pre-P1 v1 shapes still round-trip with the empty
  defaults staying off the wire.
- `model.rs` unit tests — `p1_fields_round_trip` (every new field, wire names),
  `p1_fields_default_on_pre_p1_shapes` (the additive guard over v1/v2/v3 cells + explicit nulls +
  byte-stability), `query_options_tolerates_partial_shape` (the shipped UI sends only its trio).
- `viz/time_override.rs` unit tests — replace/shift/compose/no-op/non-numeric-untouched/grammar.
- `crates/host/tests/viz_query_test.rs::panel_time_override_applies_to_target_dispatch` — end to
  end against a real booted node + really-seeded samples through the real `series.read` dispatch:
  `timeFrom` replaces the range (rows vanish), `timeShift` moves a caller range back onto them.
- Mandatory gates: no new verb in P1, so the surface is the existing `dashboard.save`/`viz.query`
  cap-deny + workspace-isolation tests — all green in the runs above.
- Suite status, honestly: every suite this diff touches is green (`dashboard_*`, `viz_query`,
  `panel`, `widget_*`, the lb-host lib's 224, the gateway dashboard routes). Three failures exist
  in this checkout that are **pre-existing/environmental, unrelated to this diff** — verified by
  running the worst one from a clean pre-session commit (`9a4b7041`) in a temp worktree, where it
  fails identically: (1) `agent_persona_catalog_test` — 6 tests die on
  `PersonaSkill { builtin.data-analyst, core.datasources }` (a persona→skill catalog gap; owner:
  next agent/personas session); (2) `lb-runtime` lib tests — the `~/.cargo` git checkout of
  `lb-ext-sdk` is missing its `wit/` dirs (broken cached checkout); (3) gateway
  `publish_install_test` — needs the prebuilt `hello_v2` wasm artifact (I rebuilt `hello`'s, which
  un-blocked `agent_decision_test` et al., but `hello-v2` isn't built here).

## Files touched

- `rust/crates/host/src/dashboard/model.rs` — `QueryOptions` + the Cell/Dashboard/Variable fields
  + unit tests.
- `rust/crates/host/src/dashboard/save.rs`, `tool.rs` — `timezone` plumbing (preserve-on-omit) +
  descriptor row.
- `rust/crates/host/src/dashboard/pin.rs`, `src/panel/validate.rs`, `src/panel/hydrate.rs` —
  `..Cell::default()` on constructors (P1 fields defaulted/cleared per the strip discipline).
- `rust/crates/host/src/viz/time_override.rs` (new), `viz/query.rs`, `viz/mod.rs` — the override.
- `rust/role/gateway/src/routes/dashboard.rs` — `timezone` in the save body.
- `rust/crates/host/tests/dashboard_query_options_test.rs` (new), `viz_query_test.rs` (+1 test),
  Cell-literal test fixtures gained `..Default::default()`.
- Docs: this file, `docs/debugging/dashboard/query-options-silently-dropped-on-save.md`,
  `docs/debugging/README.md`, `docs/STATUS.md`.
