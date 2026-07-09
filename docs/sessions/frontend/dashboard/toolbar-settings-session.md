# Dashboard toolbar clean-up + opt-in header controls

**Topic:** dashboard toolbar-settings
**Date:** 2026-07-09

## The ask

A dashboard's header (the `AppPage` actions row) carried too many controls at rest.
Trim the noise and make three of them **opt-in, hidden by default**, toggled from
the existing Page settings dialog:

- **date range** (from/to pickers) — hidden by default
- **page refresh rate** control — hidden by default
- **share** button + private/team/workspace visibility — hidden by default

## What shipped

One additive, host-opaque `toolbar` object on the dashboard record — three booleans,
all default `false` (every control hidden). Same discipline as page-settings
`icon`/`color`: a closed struct field with serde-default + preserve-on-omit through
`dashboard_save_meta`. The host never branches on the flags; it stores and returns them.

### Backend (`rust/crates/host`)

- `dashboard/model.rs` — new `Toolbar { date_select, refresh_rate, share }`
  (`Copy`, all `#[serde(default)]`, camelCase renames `dateSelect`/`refreshRate`);
  added `Dashboard.toolbar`. Unit test `toolbar_round_trips_and_defaults_off`
  (round-trip + a pre-toolbar shape defaults every flag off).
- `dashboard/save.rs` — `dashboard_save_meta` takes `toolbar: Option<Toolbar>`
  (preserve-on-omit: `None` keeps the stored flags). `dashboard_save` passes `None`.
  Added `toolbar` to the `save_descriptor` arg schema.
- `dashboard/tool.rs` — `opt_toolbar_arg` (present object ⇒ `Some`, absent/null ⇒ `None`,
  malformed ⇒ `None`, lenient).
- `dashboard/pin.rs` — fresh-dashboard literal gets `toolbar: Default::default()`.
- `mod.rs` / `lib.rs` — export `Toolbar` (as `DashboardToolbar`).
- `role/gateway/src/routes/dashboard.rs` — `SaveDashboard.toolbar: Option<DashboardToolbar>`,
  forwarded to `dashboard_save_meta`.

### Frontend (`ui`)

- `lib/dashboard/dashboard.types.ts` — `Toolbar` interface + `Dashboard.toolbar`.
- `lib/dashboard/dashboard.api.ts` — `DashboardMeta.toolbar`.
- `lib/ipc/http.ts` — **the drop point**: the hand-mapped `dashboard_save` body
  whitelists fields; added `toolbar` (forwarded only when present). Without this the
  flag never reached the gateway.
- `features/dashboard/DashboardSettingsDialog.tsx` — a "Toolbar" section with three
  `Switch` rows (`TOOLBAR_CONTROLS`). Save sends explicit booleans.
- `features/dashboard/DashboardView.tsx` — `const tb = current?.toolbar ?? {}`; the
  date pickers, `RefreshControl`, share button, visibility `Select`, and the read-only
  visibility `Badge` now each gate on their `tb.*` flag. A clean board shows none of them.

## Bugs hit + fixed (this session)

1. **IPC shim dropped `toolbar`.** `lib/ipc/http.ts` `dashboard_save` destructures a
   fixed field set; `toolbar` fell out silently (persisted `share:false` despite the
   toggle). Root cause of the first gateway-test failure. Fixed by forwarding it.
2. **Dialog re-seed wiped pending edits.** `DashboardSettingsDialog`'s re-seed
   `useEffect` depended on `[open, dashboard]`, so any parent re-render while the dialog
   was open reset local state to the stored value — a toggle flipped but not yet saved
   was silently reverted. Changed the dep to `[open]` (re-seed only on the open
   transition). Latent for the description/icon/color edits too; now fixed for all.
3. **Switch nested in `<label>` double-fired.** Initial toggle rows were `<label>`s
   wrapping the `<button role="switch">`; the label forwarded its click to the button,
   toggling on-then-off. Changed rows to plain `<div>` (the Switch carries its own
   `aria-label`).

## Tests (green)

- `cargo test -p lb-host --test dashboard_test` — 12 pass. `page_settings_round_trip_and_preserve`
  extended to set + assert + preserve the toolbar flags through `save_meta` and a plain `save`.
- `cargo test -p lb-host --lib dashboard::model` — 6 pass (incl. new toolbar test).
- `pnpm exec vitest run --config vitest.gateway.config.ts DashboardView.gateway.test.tsx` —
  13 pass, incl. new **"toolbar settings"**: share control absent by default → enable in
  dialog → appears in header → `dashboard.get` shows `toolbar.share=true`, others false →
  a plain layout save preserves it. Real spawned gateway (rule 9).
- `cargo build --workspace` + `pnpm exec tsc --noEmit` clean.

## Notes / decisions

- **One `toolbar` object, not three `ui.*` prefs.** Follows the closed-struct rule
  (`prefs-closed-struct-not-kv`, `dashboard-variable-closed-struct`): a new UI axis is
  an additive serde-default field, not a KV key — else it's silently dropped on save.
- **Flags default off** to satisfy the "hidden by default" ask AND to auto-declutter every
  existing board (a pre-toolbar record deserializes with all three off).
- The **Export** button stays always-on (it's not one of the three requested controls and
  is a low-noise authoring affordance).
