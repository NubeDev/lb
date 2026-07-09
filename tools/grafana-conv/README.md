# grafana-conv — the standalone Grafana → our-dashboard converter

A standalone tool that takes a **Grafana dashboard JSON file in** and emits **our
dashboard JSON out** (one direction: Grafana → us). Own workspace, own UI; folds
into the main project later. Scope: [`docs/scope/frontend/dashboard/grafana-conversion-scope.md`](../../docs/scope/frontend/dashboard/grafana-conversion-scope.md).

```
Grafana `.json` ──▶ mapper ──▶ { our `Dashboard` JSON, ConversionReport }
```

## Layout

| Path | What |
|---|---|
| `mapper/` | The pure `grafana_json → (Dashboard, ConversionReport)` Rust crate. No I/O. |
| `app/` | The thin Rust seam: Axum `POST /convert` (browser build). |
| `ui/` | The one-screen shadcn UI (drop/paste input → output + report). |
| `Cargo.toml` | The standalone workspace (separate from `rust/`). |

The output `Dashboard`/`Cell`/`Variable` types are a **vendored mirror** of
`rust/crates/host/src/dashboard/model.rs` (`mapper/src/model.rs`), guarded by a
mirror-sync test. The fold-in (Stage 2) deletes the mirror and path-depends on the
host crate directly — a wiring job, not a re-map.

## Build / test

```sh
# mapper + app (Rust)
cargo build --workspace
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets

# UI
cd ui
pnpm install --ignore-workspace     # standalone (the repo pnpm workspace doesn't include this dir)
pnpm test                           # component tests (renders real mapper output)
pnpm exec tsc --noEmit              # typecheck
pnpm run dev                        # dev server (proxies /convert to the app)
```

## Run it

```sh
# 1) the seam (one terminal)
cargo run -p grafana-conv-app       # serves POST /convert on 127.0.0.1:7878

# 2) the UI (another terminal)
cd ui && pnpm run dev               # the dev server proxies /convert to the seam
```

Open the printed URL, paste/drop a Grafana `.json`, hit **Convert**, get the
converted JSON + the report. Copy / download from the output pane.

## The report — the honesty contract

Every Grafana feature that appears in the input is **named** in the output's
`ConversionReport`, grouped by fate:

- **mapped** — cleanly mapped 1:1 onto a `Dashboard`/`Cell`/`Variable` field.
- **degraded** — preserved-but-not-rendered (carried as opaque data + flagged).
- **dropped** — not carried at all, named so it is a decision, not a silent loss.

The audit that drives these is in the scope ("Stage 1 — the audit"). The two
corruption traps it surfaces are handled here:

1. **Row dual-encoding** — collapsed rows nest children in `row.panels[]`;
   expanded rows leave `panels[]` empty and put children as flat `y`-ordered
   siblings. Both normalize to one `view:"row"` cell membership (golden-tested
   with one fixture of each).
2. **Chained variables** — no explicit graph in Grafana; `$var`/`${var}`/`[[var]]`
   refs are parsed out of each variable's query to emit variables in
   dependency-resolvable order. A cycle is reported, not hung.

## Regenerate the UI test fixture

The UI test renders against REAL mapper output (not a hand-written fake):

```sh
cargo run -p grafana-conv-mapper --example emit_fixture > ui/src/test/sample-output.json
```

## Scope status

**Stage 0 (this cut) — shipped.** The standalone converter is built and green:
the pure mapper crate (with golden + mirror-sync tests), the Axum serve seam, and
the one-screen UI. Datasource UID remapping, `dashboard.import` as a host verb,
and the export direction are **out of this cut** and stay in
[`viz/import-export-scope.md`](../../docs/scope/frontend/dashboard/viz/import-export-scope.md).
