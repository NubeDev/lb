# Persona run only sees 3 tools — `tools.catalog` served only the palette-descriptor subset

**Area:** agent (run assembly) / tools.catalog
**Date:** 2026-07-05
**Symptom:** An in-house run under `builtin.widget-builder` (which `extends = ["builtin.data-analyst"]`)
reported it had only THREE tools — `dashboard.catalog`, `dashboard.pin`, `federation.query` — and that
`datasource.list` "is not available in my current toolset", even though the persona's resolved
`granted_tools` union clearly advertises `datasource.*`, `series.*`, `store.query`, `query.*`, `viz.query`, …

## Root cause (two stacked gaps — neither was the persona layer)

The persona machinery was **correct**: `resolve_effective` unions the `extends` closure, and
`narrow_tools` intersects with trailing-`*` glob semantics (proven by
`rust/crates/host/tests/persona_menu_full_catalog_test.rs`, which passed against the persona layer
before any fix). The run's menu is `reachable_tools` = `tools.catalog` ∩ caller caps, THEN persona-narrowed
— and the catalog was starving the pipeline at its head:

1. **`tools.catalog` only enumerated the ~11 verbs with palette descriptors.**
   `rust/crates/host/src/tools/catalog.rs` walked `host_descriptors()` (the guided-palette rail:
   `federation.query`, `query.save/run/compile`, `agent.invoke`, `reminder.*`, `dashboard.catalog/pin`,
   `secret.*`) + extension registry entries — nothing else. So `datasource.list`, `store.query`,
   `series.*`, `viz.query`, `dashboard.save`, `flows.*`, `rules.*`, … could NEVER reach an agent menu,
   regardless of caps or persona. Intersect that with widget-builder's globs and you get exactly the
   three tools the model reported.
2. **The "authoritative" host inventory had itself drifted.** `system/catalog.rs::HOST_TOOLS` (what
   `system.tools` serves) was missing whole dispatched families — `datasource.`, `viz.`, `flows.`,
   `rules.`, `query.`, `channel.`, `prefs.`, `reminder.`, `assets.`, `telemetry.`, `layout.`,
   `message.`, `history.`/`undo`/`redo`, `federation.`, `tools.` — because its coverage test iterated a
   hand-copied prefix list that had gone stale against `tool_call.rs::is_host_native`.

## Fix

- `tools/catalog.rs`: after the rich descriptors, the catalog now also serves the full host-native
  inventory (`system::host_catalog()`), name-only rows (title = the one-line summary), filtered to
  bridge-dispatchable verbs (`tool_call::is_host_native`) and gated by the SAME per-verb
  `authorize_tool` — the catalog finally honors its own documented contract ("every tool this
  principal may run"). A verb gains an arg schema by adding a palette descriptor, as before.
- `system/catalog.rs`: added the ~70 missing `HOST_TOOLS` rows for the dispatched families.
- `tool_call.rs`: the dispatch families are now shared consts (`HOST_NATIVE_PREFIXES` /
  `HOST_NATIVE_EXACT`); `is_host_native` iterates them and the inventory coverage test derives from
  them — the hand-maintained mirror that caused gap 2 cannot drift silently again.

**Rejected alternative:** having `reachable_tools` enumerate verbs from a second source instead of
widening `tools.catalog`. That would fork "what can I run" into two truths; the catalog is documented
as *the* honest menu (`/`-palette and agent alike), so it is the layer that had to be fixed.

## Regression test

`rust/crates/host/tests/persona_menu_full_catalog_test.rs`:
- `reachable_tools_serves_full_host_inventory_and_persona_keeps_it` — a member holding the
  data-analyst read caps gets `datasource.list`, `store.query`, `series.read`, `viz.query`, … in
  `reachable_tools`, an ungranted verb stays absent (the wall), and the widget-builder persona's
  narrowed menu keeps the parent surface.
- `widget_builder_unions_data_analyst_surface` — the `extends` union + glob narrowing in isolation.
- `system::catalog::tests::host_catalog_covers_dispatch_prefixes` — now derived from the dispatcher's
  own consts.

## Note for live nodes

A model may still *under-report* its menu in prose; the honest check is `tools.catalog` for the caller
(or the run's advertised tool list in the gateway request log). Also remember the dev node does not
hot-reload Rust — a node binary older than this fix still serves the starved catalog.

Schema note: inventory-row tools are advertised without arg schemas until they grow a descriptor —
the model can call them but may guess argument names; add a `descriptor()` per verb as they become
palette/agent-critical (see tool-schema-dropped-so-model-asks-in-prose.md for why schemas matter).
