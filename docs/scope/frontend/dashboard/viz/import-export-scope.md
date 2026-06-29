# Viz scope — Grafana JSON import / export (the interchange edge)

Status: scope (the ask). Part of the [`viz/`](README.md) slice — the **interop boundary** that turns a
Grafana dashboard JSON into our native [`Cell`/`Dashboard`](panel-model-scope.md) record and back.
Promotes to [`public/frontend/dashboard.md`](../../../../public/frontend/dashboard.md) when it ships
(Phase 4 of the umbrella).

One paragraph: deliver the user's literal ask — **"export a dashboard from Grafana as JSON and import
here, and back."** We add **two host verbs** — `dashboard.import {json}` and `dashboard.export {id}` — and
**one bidirectional mapper** (the heart of this doc) that translates a Grafana dashboard JSON ↔ our native
panel-model record. We never store Grafana JSON raw (the [spine](panel-model-scope.md) decision): the
mapper runs at the edge, the record stays ours. Import first runs the incoming JSON through a bounded
**`schemaVersion` migration** (a ported subset of Grafana's sequential migrations) so an old dashboard
normalizes to the shape the mapper understands, then maps panel-by-panel, then returns a **report** that
asks the user to **remap every referenced datasource onto one of *our* registered datasources** (the
tenancy-critical step — a Grafana `uid` means nothing here, and a ws-B import can never bind a ws-A
datasource). Anything we don't support — a panel type, a transform, a variable type, an unmapped
datasource — is **preserved on the cell and rendered as an honest "unsupported: &lt;type&gt;" placeholder**,
never silently dropped and never faked, so a supported dashboard **round-trips** semantically stable.

## Goals

- **Two MCP verbs (§6.1):** `dashboard.import {json} -> {id, report}` and `dashboard.export {id} -> {json}`,
  each a **bounded, synchronous, single-document** op (one dashboard in / one dashboard out).
- **One bidirectional mapper** — `grafana→cell` (import) and `cell→grafana` (export) — that maps the full
  supported panel/field/transform/variable surface 1:1 onto our additive record.
- **`schemaVersion` migration on import** — normalize an older Grafana JSON to the version the mapper reads,
  by porting the *subset* of Grafana's migrations the supported set needs (not all 42).
- **Datasource remapping on import** — the `report` enumerates every referenced datasource and the user
  binds each to one of *our* datasources (or native/series), inside the workspace wall; never auto-trusted.
- **Honest degradation** — unsupported types/transforms/variables/unmapped sources are preserved (so
  re-export round-trips) and shown as a named placeholder; the `report` lists everything degraded.
- **Round-trip fidelity** — a supported Grafana JSON → import → export → JSON is semantically stable
  (modulo our additive fields), via a bounded passthrough of unknown fields.
- **Additive, no new datastore, no leaked secrets** — Grafana JSON is interchange, never raw storage; a
  Grafana datasource's credentials are *never* imported.

## Non-goals

- **Not Grafana's full migration chain.** We port the migrations our supported panel/field set needs
  (panel-type renames, fieldConfig normalization, datasource-uid resolution) — not all V2..V42.
- **Not a bulk/library import.** One dashboard per call. A multi-dashboard or folder import is a **job**
  (state: a named follow-up — `dashboard.import_bulk` over [`../../../jobs/jobs-scope.md`](../../../jobs/jobs-scope.md)), not this synchronous verb.
- **Not Grafana provisioning / API parity.** No `/api/dashboards/db`, no Grafana datasource provisioning,
  no library-panel resolution beyond inlining what the JSON carries.
- **No raw-JSON storage.** We don't fork the record to keep Grafana's schema (the [spine](panel-model-scope.md)
  rejected that); only a *bounded* `_grafana` passthrough blob per cell for unknown-field round-trip.
- **No credential import.** A Grafana datasource's connection/secret is out of scope; we map only to an
  already-registered *our* datasource whose secret is server-side ([datasources](../../../datasources/datasources-scope.md)).

## Intent / approach

**One mapper at the edge, two directions, one responsibility per file.** Import is a 3-stage pipeline:
**migrate** (normalize `schemaVersion`) → **map** (`grafana→cell`, panel-by-panel) → **report** (datasource
remap prompts + degraded list). Export is the inverse `cell→grafana` map, re-emitting the bounded
passthrough so unknown fields survive. The mapper consumes the *taxonomy* defined by the sibling scopes
(view ids, `FieldConfig`, `Transformation`, `DataSourceRef`) so the map is mechanical, not inventive.

**Rejected: store Grafana JSON verbatim and render from it.** It forks our `Cell`/`Dashboard` record,
bypasses the serde-default additive `v`/`schemaVersion` discipline, and couples our store to Grafana's
`schemaVersion` churn (currently 42 and climbing) — the same rejection the [umbrella](README.md) and
[spine](panel-model-scope.md) already made. We own the record; Grafana JSON is interchange mapped at the
boundary, so a v1/v2/v3 cell is never broken by a Grafana schema bump.

**Rejected: auto-bind imported datasource uids by name match.** A Grafana `uid` (e.g. a Prometheus or MySQL
uid) is meaningless and untrusted here; silently matching it to one of our datasources would let an import
*look* wired while pointing at the wrong source — or worse, leak across the workspace wall. So import
**asks**: the `report` forces an explicit remap, and unmapped panels degrade honestly.

## How it fits the core

- **Tenancy / isolation (rule 6):** the **workspace comes from the caller's token, never from the JSON** —
  an imported dashboard's `title`/`uid`/`org` carry no authority. Every datasource the JSON references is
  resolved **only** against the importer's workspace (`datasource.list` for that ws); a ws-B import can
  **never** name or bind a ws-A datasource. This is the **headline isolation test**. The created dashboard
  is a normal workspace-scoped `dashboard:{id}` record.
- **Capabilities (rule 5/7):** new gated verbs.
  - `dashboard.export` → `mcp:dashboard.export:call` (a read; deny is opaque).
  - `dashboard.import` is a **write** → needs **both** `mcp:dashboard.import:call` **and** the editor save
    cap `mcp:dashboard.save:call` (it creates a dashboard). Datasource remapping additionally requires the
    user already hold the target datasource's grant — you can only map to a source you may use.
- **Placement (rule 1):** `either` — the mapper is pure shared code; one transport (Tauri `invoke` / gateway
  SSE+HTTP), no `if cloud`. Migration + map run host-side on import; export serializes host-side.
- **MCP surface (§6.1):** two verbs, both **bounded synchronous single-document** ops (the §6.1 get/CRUD
  shape, *not* the batch-as-job shape). Bound: one dashboard, capped panel/field/transform counts inherited
  from the [spine](panel-model-scope.md). A bulk import would be the §6.1 batch-as-job follow-up named above.
- **Data (SurrealDB):** import creates one `dashboard:{id}` record (the same UPSERT `dashboard.save` writes);
  no new table, no raw JSON stored. The only new persisted bytes are the **bounded `_grafana` passthrough
  blob** per cell (capped, e.g. ≤8 KB/cell) holding unknown Grafana fields for export fidelity.
- **Bus (Zenoh):** none — import/export are request/response state ops; live data still streams over the
  shipped SSE at render time. State vs motion holds.
- **Sync / authority:** the imported dashboard rides the shipped `(table,id)` UPSERT on the §6.8 sync path
  like any saved dashboard; additive fields + the passthrough blob replay idempotently.
- **Secrets:** **none cross.** A Grafana datasource's credentials in the JSON are ignored; we import only
  the *mapping* to our already-registered datasource, whose DSN/secret is server-side (datasources doctrine).
- **SDK/WIT impact:** two new host verbs in the dashboard MCP surface; the **cell/record contract is
  unchanged** beyond the additive `_grafana` passthrough field (serde-default), already covered by the
  [spine](panel-model-scope.md)'s v3 bump.

## The mapper (the interop contract)

One responsibility per file; `grafana→cell` (import) and `cell→grafana` (export) are symmetric inverses.

| Grafana JSON | Our record | Mapping note |
|---|---|---|
| `panel.type` | `Cell.view` | alias table on import: `graph→timeseries`, `singlestat→stat`/`gauge`, `table-old→table`; reverse on export. Unsupported type → preserved + placeholder. [`chart-types-scope.md`](chart-types-scope.md) |
| `panel.datasource` + `panel.targets[]` | `Cell.sources[]` + each `Target.datasource`/`tool`/`args` | a target = one query; the `DataSourceRef.uid` is **remapped** (see below). [`datasource-binding-scope.md`](datasource-binding-scope.md) |
| `panel.fieldConfig{defaults,overrides[]}` | `Cell.fieldConfig` | unit/decimals/thresholds/mappings/color/overrides 1:1. [`field-config-scope.md`](field-config-scope.md) |
| `panel.transformations[]` | `Cell.transformations[]` | `{id,options,disabled,filter}` 1:1; unsupported `id` → preserved + flagged. [`transformations-scope.md`](transformations-scope.md) |
| `panel.gridPos{x,y,w,h}` | `Cell.x,y,w,h` | exact — both 24-col (the [spine](panel-model-scope.md) pins our grid to 24). |
| `panel.options` | `Cell.options` | per-view structured options, passed through per the view's shape. |
| `panel.id` (numeric) + `panel.pluginVersion` | mapper-carried `id` + `Cell.pluginVersion` | numeric `id` round-trips inside the mapper; our record keys cells by string `i`. |
| `templating.list[]` (VariableModel) | `Dashboard.variables` | map Grafana var types (`query`/`custom`/`constant`/`textbox`/`interval`/`datasource`/…) to our shipped vars lib; unsupported type → preserved + flagged. |
| `time`/`refresh`/`tags`/`title`/`description` | dashboard fields (time/refresh live in routing) | mapped onto the record where we have a home; the rest passes through. |
| unknown panel/dashboard fields | bounded `_grafana` passthrough blob | re-emitted on export for round-trip fidelity. |

### `schemaVersion` migration on import

Before mapping, run the incoming JSON through a migration step that normalizes an old Grafana
`schemaVersion` (its own version, lives only in the JSON — distinct from `Dashboard.schemaVersion`, *our*
doc version, and from `Cell.v`, the contract version) up to the version the mapper understands. We **port
the subset** Grafana applies for our supported set — the panel-type renames (`graph→timeseries`,
`singlestat→stat`/`gauge`), the `fieldConfig` standardization, and datasource-uid resolution — modeled on
`apps/dashboard/pkg/migration/schemaversion/migrations.go` (sequential `V2()..V42()`), **not** all 42.
A `schemaVersion` newer than we understand, or a field a ported migration doesn't cover, **degrades
honestly** (preserved + flagged in the report), never guessed. State the bound in the report.

### Datasource remapping (the tenancy-critical step)

A Grafana dashboard references datasources by `DataSourceRef{type,uid}`. On import we **cannot auto-trust**
those uids. The mapper collects every referenced `(type,uid)`, and the import `report` lists each with a
**dropdown of *our* registered datasources** (`datasource.list` for the caller's workspace) plus
native/series options. The user maps each one; anything left unmapped marks the owning panels
**"unmapped"** — they render an honest "datasource not mapped" state (never fake data). The remap target
must be a datasource **in the importer's workspace** that the importer holds a grant for — the workspace
wall and the cap check are enforced server-side, not by the JSON.

## Example flow

1. User opens **Import dashboard** in the dashboard roster toolbar, pastes Grafana JSON (or uploads a
   `.json`). The UI calls `dashboard.import {json}` (dry-run/preview mode).
2. Host derives the **workspace from the token**, runs the JSON through `schemaVersion` migration
   (normalizing e.g. an old `graph` panel to `timeseries`), then `grafana→cell` maps each panel.
3. Host returns a **`report`**: mapped panels, a **datasource-remap row per referenced uid** (each with a
   dropdown of this workspace's `datasource.list` + native/series), and a **degraded list** (unsupported
   panel types, transforms, variable types — preserved, flagged).
4. The UI renders the preview: panel count, the remap dropdowns, the degraded warnings. The user maps each
   datasource to one of *our* sources and clicks **Confirm**.
5. The UI re-calls `dashboard.import {json, mappings}` (commit). Host re-checks `mcp:dashboard.import:call`
   ∩ `mcp:dashboard.save:call` ∩ each mapped datasource's grant, builds the `Cell`/`Dashboard` record (with
   the bounded `_grafana` passthrough for unknown fields), and UPSERTs it → returns `{id, report}`.
6. The dashboard opens; unmapped/unsupported panels show their named placeholder; everything supported
   renders live.
7. Later the user clicks **Download JSON** (export). Host re-checks `mcp:dashboard.export:call`, runs
   `cell→grafana`, re-emits the passthrough blob, and returns the JSON — semantically equal to step-1 input
   for the supported subset.

## Testing plan

Per [`../../../testing/testing-scope.md`](../../../testing/testing-scope.md) — real gateway, real store,
seeded real rows, **no `*.fake.ts`**. (A genuine Grafana export `.json` is a *fixture file*, not a fake
backend — it's the real interchange artifact; the gateway/store/mapper all run for real.)

- **Round-trip fidelity (the headline):** a real supported Grafana export JSON → `dashboard.import` →
  `dashboard.export` → JSON is **semantically stable** (deep-equal modulo our additive fields), proving the
  mapper + passthrough.
- **`schemaVersion` migration:** an old-`schemaVersion` JSON with a `graph`/`singlestat` panel imports as
  `timeseries`/`stat`; a too-new `schemaVersion` degrades honestly (flagged, not crashed).
- **Datasource remapping + workspace isolation (mandatory):** a JSON referencing an unknown datasource uid
  produces a remap prompt; mapping to a **ws-B** datasource from a **ws-A** import is **rejected** (the hard
  wall); the two-session test asserts a ws-B import can never name a ws-A datasource.
- **Capability deny (mandatory):** `dashboard.import` without `mcp:dashboard.import:call` **or** without
  `mcp:dashboard.save:call` is denied opaquely; `dashboard.export` without its cap is denied; mapping to a
  datasource the caller lacks a grant for is denied.
- **Honest degradation:** an unsupported panel type / transform / variable type is **preserved** (survives
  re-export) and listed in the `report`, and renders the named placeholder — never dropped, never faked.
- **Bounds:** an oversized `_grafana` passthrough is capped (not stored unbounded); a multi-dashboard
  payload is rejected with "use bulk import (job)" — the single-document bound holds.

## Risks & hard problems

- **The mapper is the interop contract.** A wrong panel/field map silently corrupts a dashboard on import;
  the round-trip test is the guardrail. Map once, test both directions per row.
- **`schemaVersion` migration drift.** Porting a subset means a Grafana version we haven't covered can
  arrive; the "newer than understood → degrade honestly" rule keeps it from corrupting. Pin the highest
  `schemaVersion` we migrate to and surface it in the report.
- **Datasource remap is where isolation can leak.** The remap must resolve strictly within the caller's
  workspace and grant set — a single bypass breaks the hard wall. Enforce server-side, test across sessions.
- **Passthrough blob growth / staleness.** An unbounded `_grafana` blob bloats the record; a stale blob can
  re-emit fields that conflict with edited cell fields on export. Cap it, and let *mapped* fields win over
  passthrough on export (passthrough fills only gaps).
- **Lossy round-trips look like data loss.** Degraded fields must be *visible* (report + placeholder), or a
  user thinks import worked when it half-did. Trust hinges on honesty here.

## Resolved decisions

No blocking open questions — these are the long-term answers the build follows.

- **Preserve unknown Grafana fields as a per-cell, bounded `_grafana` passthrough blob** — the unknown
  bytes sit next to the panel they belong to and growth is bounded per cell. Mapped fields win over
  passthrough on export (passthrough fills only gaps). Revisit only if dashboard-level unknowns prove common.
- **One `dashboard.import` verb, two phases** (not a separate preview verb): preview returns `{report}`
  with no write; commit takes the chosen `mappings` and UPSERTs. Fewer verbs, fewer caps — the preview is
  just import without the write.
- **Port only the Grafana migrations the supported set needs** for Phase 4 — panel renames
  (`graph`→`timeseries`, `singlestat`→`stat`/`gauge`), `fieldConfig` standardization, and datasource-uid
  resolution; grow the port as the supported chart/field/transform set grows.
- **Bulk/folder import is later, as a job** (`dashboard.import_bulk` over
  [jobs](../../../jobs/jobs-scope.md)); this scope stays single-document and synchronous (§6.1 bound).

## Related

- [`README.md`](README.md) — the viz umbrella (Phase 4 + the reconciliation table) ·
  [`panel-model-scope.md`](panel-model-scope.md) — the spine (the `Cell`/`Dashboard` shape, the two version
  fields, the 24-col grid this map relies on).
- Siblings the mapper consumes: [`chart-types-scope.md`](chart-types-scope.md) (`view` set + aliases) ·
  [`field-config-scope.md`](field-config-scope.md) (`FieldConfig` map) ·
  [`transformations-scope.md`](transformations-scope.md) (`Transformation` map) ·
  [`datasource-binding-scope.md`](datasource-binding-scope.md) (`DataSourceRef` + the remap target) ·
  [`panel-editor-scope.md`](panel-editor-scope.md) (the editor the imported dashboard opens in).
- [`../widget-builder-scope.md`](../widget-builder-scope.md) — the v2 contract the record extends ·
  [`../../dashboard-scope.md`](../../dashboard-scope.md) — the dashboard scope this roster/toolbar lives in.
- [`../../../datasources/datasources-scope.md`](../../../datasources/datasources-scope.md) — the
  `datasource.list`/`datasource.*` plane the remap binds to (workspace-scoped) ·
  [`../../../jobs/jobs-scope.md`](../../../jobs/jobs-scope.md) — where a future bulk import runs ·
  [`../../../testing/testing-scope.md`](../../../testing/testing-scope.md) — the real-gateway test doctrine.
- Public stub: [`../../../../public/frontend/dashboard.md`](../../../../public/frontend/dashboard.md).
- Grafana reference clone `/tmp/grafana` — `kinds/dashboard/dashboard_kind.cue` (dashboard/panel/VariableModel
  shapes, `schemaVersion` 42), `apps/dashboard/pkg/migration/schemaversion/migrations.go` (the sequential
  migrations we port a subset of).
- README **§6.1** (get/CRUD/batch-as-job API shape), **§6.6** (the three gates), **§3** (rules 2/6/7).
