# Dashboard page settings scope

Status: **SHIPPED (2026-07-09)**.

## The ask

Give a dashboard *page* a small set of presentation settings — a per-page **icon**, an **icon colour**,
and a one-line **description** (subtitle) — and make the previously-hardcoded header blurb
("Live workspace dashboards and series widgets.") the *default* for that description rather than a fixed
string. The author edits these from a **Page settings** dialog in the dashboard header (admin/edit-cap
gated); a viewer just sees the result.

## The model (additive, host stays opaque)

Three additive, serde-defaulted fields on the `dashboard:{id}` record (`host/src/dashboard/model.rs`):

- `description: String` — the subtitle under the page title. Empty ⇒ the UI's default blurb.
- `icon: String` — a stable icon-lib name (resolved via `ui/src/lib/icons`, fallback `layout-dashboard`).
- `color: String` — any CSS colour for the icon chip. Empty ⇒ the shell accent.

`icon` + `color` also ride the cheap `DashboardSummary` so the roster paints them **without a full get**.

A pre-page-settings dashboard round-trips byte-clean (every field `#[serde(default)]`). Opaque to the
host beyond serde — never branched on.

## The write path — preserve on omit (like `visibility`)

The three fields follow the **preserve-on-omit** discipline `visibility` already uses: a plain
`dashboard.save` (layout or variable edit) that does **not** carry a settings key **preserves** the
stored value, so a drag/resize/add-cell save never blanks the page chrome. Only the settings dialog
sends them.

- Host: `dashboard_save_meta(…, description: Option<String>, icon, color, …)` is the full form; the
  existing `dashboard_save(…)` is a thin wrapper passing `None, None, None` (so the ~40 existing
  callers/tests are untouched). `None` = preserve, `Some` = set.
- Gateway `POST /dashboards` (`role/gateway/src/routes/dashboard.rs`): the three fields are
  `Option<String>` `#[serde(default)]` and forwarded to `dashboard_save_meta`.
- MCP `dashboard.save` (`dashboard/tool.rs` + `save_descriptor`): the three keys are advertised as
  optional strings; `opt_str_arg` maps present-string → `Some`, absent/null → `None` (preserve).

## The UI

- `DashboardSettingsDialog.tsx` — the modal: description input, an icon-colour row (preset swatches +
  native custom picker + reset), and the shared `IconPicker`, with a live header-chip preview.
- `AppPage`/`AppPageHeader` grew an optional `iconColor` prop — a themed page tints the icon chip with
  the chosen colour instead of the shell accent tokens (no colour ⇒ unchanged).
- `RosterItem` grew optional `icon`/`iconColor` — a per-row icon override (falls back to the rail's
  shared icon). Generic; the dashboard roster passes the summary's `icon`/`color`.
- Export/import (`portable.ts`) carries the three fields for round-trip fidelity.

## Tests

- `host/src/dashboard/model.rs` — round-trip + summary-carries + pre-settings-additivity unit tests.
- `host/tests/dashboard_test.rs::page_settings_round_trip_and_preserve` — set via `dashboard_save_meta`,
  read back through get + list, then a plain `dashboard_save` **preserves** it (and a single-field meta
  set preserves the others).
- `DashboardView.gateway.test.tsx` — the real-node path: edit the description in the dialog, it renders
  in the header + persists, and a subsequent layout save preserves it.
- `portable.test.ts` / `DashboardRoster.test.tsx` — green (roster + bundle shapes).

## Rejected alternatives

- **A new `dashboard.set_meta` verb.** More surface for no gain — the settings are just three more fields
  on the one record; `dashboard.save` already owns the write, and preserve-on-omit keeps layout saves
  from clobbering them.
- **Changing `dashboard_save`'s positional signature.** Would have forced edits across ~40 call sites
  and every test; the `dashboard_save_meta` full-form + preserving wrapper avoids that entirely.
