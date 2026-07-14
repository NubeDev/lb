# viz Grafana-parity — Phase 4: JSON import / export (the interop edge)

**Scope:** `docs/scope/frontend/dashboard/viz/import-export-scope.md` (the umbrella's Phase 4).
**Date:** 2026-07-14. **Status:** SHIPPED (uncommitted working tree). 48 new tests green, no regressions.

## What P4 asked for

The user's literal ask: *"export a dashboard from Grafana as JSON and import here, and back."* Two
host verbs + one bidirectional mapper that translates a Grafana dashboard JSON ↔ our native
`Cell`/`Dashboard` record, consuming the P3 `grafana-map` migration pin. All the scope's decisions
were already resolved — this session built them.

## What shipped

**One additive model field + a bound** (`crates/host/src/dashboard/`):
- `Cell.grafana_passthrough` (`_grafana`, `crates/host/src/dashboard/model.rs`) — the bounded blob of
  unknown Grafana panel fields, skip-if-null so a non-imported cell stays byte-stable. Opaque to the
  host + every renderer.
- `MAX_GRAFANA_PASSTHROUGH = 8 KB/cell` + `check_passthrough_bounds` in `bounds.rs`, folded into
  `check_cell_bounds` — an oversized blob is **rejected on save**, not stored unbounded.

**The bidirectional mapper** — new module `crates/host/src/dashboard/grafana/`, one responsibility
per file:
| file | responsibility |
|---|---|
| `mod.rs` | module surface + the report types (`ImportReport`/`DatasourceRemap`/`DegradedItem`) + the `&Node` MCP bridge `call_dashboard_grafana_tool` |
| `view_alias.rs` | `panel.type` ↔ `view` id alias table (both directions; legacy aliases too) |
| `to_cell.rs` | `grafana→cell` — one panel → `Cell` (gridPos, type→view, targets→sources, fieldConfig/transforms/options, unknown→passthrough) |
| `to_grafana.rs` | `cell→grafana` — the inverse; passthrough first, **mapped fields overlay it** |
| `datasources.rs` | collect referenced `(type,uid)` + apply the caller's remap (tree rewrite) |
| `import.rs` | the `dashboard.import` verb (2-phase) + `import_descriptor` |
| `export.rs` | the `dashboard.export` verb + `export_descriptor` |

**Two host verbs**, wired end to end:
- `dashboard.import {json, mappings?, id?, now}` — 2-phase (preview without `mappings` → `{report}`
  no write; commit with `mappings` → UPSERT via `dashboard_save_meta`). Needs BOTH
  `mcp:dashboard.import:call` (author) AND `mcp:dashboard.save:call` (the two-gate write).
- `dashboard.export {id} -> {grafana json}` — a read: `mcp:dashboard.export:call` (viewer) + the
  three-gate `dashboard.get`.

**Wiring** (the full reachability map):
- Dispatch: `tool_call.rs` — a dedicated branch BEFORE the store-only `dashboard.` branch (import
  needs the full `&Node` for the datasource-list remap check, like `dashboard.catalog`).
- Catalog: `tools/descriptor.rs` (schema'd descriptors) + `system/catalog.rs` (host-tool rows).
- Caps: `authz/builtin_roles.rs` — `export` on the viewer tier (a read), `import` on the member tier
  (a write). Gateway dev claims flow from `viewer_role_caps()`/role caps, so no hardcoded list to edit.
- Gateway REST: `POST /dashboards/import` + `GET /dashboards/{id}/export` (`role/gateway/`), plus the
  generic `POST /mcp/call` bridge reaches them by construction.

## The tenancy-critical step (the hard wall)

The workspace comes from the **caller's token, never the JSON** — an imported `uid`/`org`/`title`
carries no authority. On commit, every datasource `mapped_to` is verified against
`datasource.list` for the caller's workspace (which authorizes `mcp:datasource.list:call` + the
workspace wall). A target not in the caller's list → the whole import is **refused (`403`)**. A ws-B
datasource is invisible in ws-A, so a ws-A import can never bind it. Proven by
`ws_b_import_cannot_bind_ws_a_datasource` (two workspaces, one node, real seeded datasource).

## Honest degradation (nothing silently dropped)

- An **unsupported panel type** (heatmap, logs, text, a plugin panel) imports as the shipped `json`
  placeholder view with `options.unsupportedType = "<type>"` + the full original panel in `_grafana`;
  the report flags it; on export it re-emits its ORIGINAL type (a re-import degrades identically).
- An **unmapped datasource** leaves its panels' refs untouched (honest empty at render) + a report
  line. **Unsupported variable types** are preserved + flagged. The **migration** notice (from the
  P3 pin's `degraded`) rides the report too.
- Why `json` not a new `unsupported` view: adding a catalog view would drift the UI renderer switch
  and the `check_view_cells` accept-set; `json` is a real built-in that renders the raw data honestly.
  Pinned as a deliberate choice.

## Round-trip fidelity

`to_grafana` re-emits the `_grafana` passthrough FIRST, then overlays the mapped typed fields —
**mapped fields win** (the scope's "passthrough fills only gaps"), so a stale blob can't shadow an
edited field. A supported Grafana JSON → import → export → JSON is semantically stable (migration
applied, unknown fields survived). Proven by `import_preview_commit_export_round_trip`.

## Tests (48 new, no regressions)

- `grafana-map` unchanged: 29/29 (the P3 pin the import consumes).
- Host mapper units: **12** (`dashboard::grafana::*` — alias both directions, panel map, unsupported
  degrade, round-trip, passthrough-wins, datasource collect/apply).
- Host bounds units: **3** (`_grafana` cap: empty ok, oversized rejected, folded into cell bounds).
- Gateway integration (real gateway/store, real seeded datasource, Grafana JSON = fixture): **4** —
  preview→commit→export round-trip (migration + degrade + passthrough), **ws-isolation wall**,
  **caps-deny** (import needs its cap even holding save; export needs its cap), v2-app-platform
  rejected (`400` with a pointer).
- No regressions: `dashboard::*` 34, `authz::builtin_roles` 7, gateway `credentials` 3,
  `dashboard_routes_test` 6 — all green after the cap additions. `cargo clippy` clean on all new
  files; `cargo fmt` clean.

## Rule-10 / core-purity check

The mapper branches on Grafana panel-type/datasource **vocabulary** only (opaque data) — never an
extension id, datasource name, or role. Grafana JSON is interchange (serde_json Values), never
stored raw; only the bounded `_grafana` passthrough persists. Import reaches datasources through the
generic workspace-walled `datasource.list`; commit reuses the generic `dashboard_save` chokepoint.

## Not done / follow-ups (named, not silent)

- **`type`-fill on remapped datasource refs**: import writes `{uid: <our-name>}`; the concrete MCP
  `tool` (`store.query`/`series.read`/`federation.query`) is left empty for the datasource-binding
  step (the editor/`viz.query` resolves it from the bound datasource kind). This matches how a
  natively-authored v3 target carries `tool` — not a gap, a boundary.
- **Bulk/folder import** stays a future job (`dashboard.import_bulk`) — this verb is single-document.
- **The import UI** (paste/upload + the remap dropdowns + the degraded warnings) is the downstream
  rubix-ai half (`grafana-parity-ui-scope.md`); P4 delivers the backend it calls.
- **`__elements` library-panel → `panelRef` mapping**: the P3 pin strips the envelope; wiring library
  panels to ref cells on import is a follow-up (library-panels scope owns the ref shape).
- Grafana reference clone still absent — the mapping table + migration subset were built from the
  scope's pinned descriptions; worth a re-verify against real Grafana source if the clone returns.

## Git

Left untouched per standing instruction. New: the `grafana/` module, the gateway routes/handlers,
`dashboard_grafana_test.rs`, the `_grafana` field + bounds, the caps, the descriptor/catalog rows.
