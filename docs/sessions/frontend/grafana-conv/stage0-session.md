# grafana-conv Stage 0 — session log

Scope: [`docs/scope/frontend/dashboard/grafana-conversion-scope.md`](../../../scope/frontend/dashboard/grafana-conversion-scope.md) (Stage 0 — the
standalone Grafana → our-dashboard JSON converter).

## What shipped

The standalone converter is built and green: a pure `mapper` Rust crate (bytes-in
→ bytes-out), a thin Axum `app` seam (`POST /convert`), and a one-screen shadcn UI.
Lives at [`tools/grafana-conv/`](../../../../tools/grafana-conv/) as its own Cargo
workspace + its own UI, outside `rust/crates/*` and `ui/src` — per the scope's
"Placement — standalone mini-project" decision.

### mapper crate (`mapper/`)

- `src/lib.rs` — `convert(grafana_json) -> (Dashboard, ConversionReport)`. Pure; no
  I/O. Strips the `/api/dashboards/uid` envelope; rejects v2 kind-based layout.
- `src/model.rs` — **vendored mirror** of `rust/crates/host/src/dashboard/model.rs`,
  header-noted + guarded by `tests/mirror_sync.rs` (the fold-in never discovers a
  drifted shape). `#[rustfmt::skip]` keeps cargo fmt off it; `#[allow(dead_code,
  clippy::derivable_impls)]` because it mirrors the host's full surface, not just
  the bits the mapper emits.
- `src/input.rs` — the Grafana input schema (loose serde, `rename_all = "camelCase"`
  + `#[serde(flatten)] other` for everything not modelled by name).
- `src/panels.rs` — panels → `Cell`s. Handles **trap #1 (row dual-encoding)**:
  collapsed rows nest children in `row.panels[]`, expanded rows leave them as flat
  `y`-ordered siblings; both normalize to a `view:"row"` cell + flat children.
  Panel types are **opaque** (rule 10) — unknown types degrade to a `template`
  placeholder, never a branch on our ext ids.
- `src/variables.rs` — `templating.list[]` → `Variable[]`. Handles **trap #2**:
  chained-variable order is derived by parsing `$var`/`${var}`/`[[var]]` refs and
  topo-sorting; a cycle is reported, not hung.
- `src/settings.rs` — dashboard-level fields (title/time/refresh/tags mapped;
  timezone/calendar/liveNow/preload/editable/annotations/links/graphTooltip
  degraded/dropped with report lines).
- `src/report.rs` — `ConversionReport { mapped, degraded, dropped }`, the
  headline honesty deliverable.

### app seam (`app/`)

Axum `POST /convert` with `{ grafana }` body → `{ dashboard, report }`. Permissive
CORS so the dev UI hits it directly. Has a `oneshot` test driving the real mapper
through the route.

### UI (`ui/`)

One screen: drop/paste Grafana JSON on the left → converted output (copy/download)
+ grouped report on the right. Minimal shadcn-style primitives (button / textarea /
card / report). Calls the seam over `/convert` (proxied in dev). The component test
renders against REAL mapper output captured as a fixture
(`src/test/sample-output.json`) — regenerable by
`cargo run -p grafana-conv-mapper --example emit_fixture`.

## Decisions resolved (scope "Open questions")

- **Serve seam shape.** Resolved: Axum `POST /convert` for the browser build, the
  same `mapper` crate behind it; a Tauri command wrapper for desktop is the future
  wrap (one native crate behind both, the scope's recommended first step).
- **Vendor vs path-dep.** Resolved: vendored mirror (matches the standalone ask;
  one file to re-sync). `tests/mirror_sync.rs` guards drift.
- **Report surface.** Resolved: grouped by fate (`mapped`/`degraded`/`dropped`),
  matching the audit matrix the user reasons about; each line carries a stable
  `code` + `at` + `reason`.

## Testing — green

Per `docs/scope/testing/testing-scope.md`: no `*.fake.ts`, no re-implemented node
behaviour. A real Grafana export JSON is a fixture, not a fake backend; the mapper
is a pure function, so the bulk of the test surface is real-fixture → asserted
output. Capability-deny + workspace-isolation gates do NOT apply to this cut (no
token/workspace/cap in play) — they attach at the fold-in.

```
Rust (cargo test --workspace):  20 passed
  - mapper unit tests (11): model round-trips, report, variable topo-order, lib smoke
  - golden fixtures (5):    fleet-overview, rows-collapsed, rows-expanded, advanced-vars, report-completeness
  - mirror-sync guard (1):  vendored model.rs == host's, byte-for-byte
  - app route tests (2):    convert maps; v2 rejected
  - doctest (1)
Rust: cargo fmt --all --check clean; cargo clippy --workspace --all-targets clean.
UI:   vitest 2 passed; tsc --noEmit clean; vite build clean.
```

The two conversion traps are exercised:
- **Row dual-encoding** — `rows-collapsed.json` (nested children) and
  `rows-expanded.json` (flat siblings) both normalize to identical cell layouts.
- **Chained-variable ordering** — the advanced-variables fixture has `host` before
  `region` in source order with `host`'s query referencing `$region`; the test
  asserts `region` precedes `host` in the output.

## What's NOT here (deferred per scope)

- **Export direction** (our JSON → Grafana) — Stage 3.
- **`dashboard.import` host verb + cap + workspace-scoped save** — Stage 2
  (`viz/import-export-scope.md`); the standalone tool owns no token/store.
- **Datasource UID → federation remapping** — reported degraded (`datasource.uid`);
  resolution is the fold-in's job.
- **`schemaVersion` migration** — read as-is; older schemas reported degraded.
- **Tauri desktop packaging** — the Axum seam + browser UI ship first; Tauri wraps
  the same `mapper` crate (the seam decision above).

## Fold-in (Stage 2) — what changes

1. Delete `mapper/src/model.rs`; depend on `lb-host`'s `dashboard::model` types
   directly.
2. Re-point the mapper imports at the host types.
3. Delete `tests/mirror_sync.rs` (no longer a mirror to guard).
4. Expose the mapper behind `dashboard.import` (cap `mcp:dashboard.import:call`,
   workspace-scoped save) per the mapper scope.
