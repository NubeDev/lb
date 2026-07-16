# Session: roster scan paging — sweep the single-page reads outside flows (#69)

- Date: 2026-07-15
- Ask: the [#69](https://github.com/NubeDev/lb/issues/69) follow-up filed by the flows read-back
  hardening session — the same single-`lb_store::scan`-page pattern lives outside flows
  (rules/dashboard/panel/report/nav/brand/render_templates/insight); sweep them onto a paginated
  drain.
- Scope grounding: `docs/debugging/flows/single-scan-page-drops-rows-past-200.md` (the sibling fix),
  `rust/crates/store/src/scan.rs` (the paged `scan` contract).

## What changed

### 1. One canonical drain, in one place

Promoted the cursor loop to `lb_store::scan_all` (`rust/crates/store/src/scan_all.rs`) — drains
every page of a ws-scoped table in id order and returns the raw `Row`s. This is the single seam
every "read the whole table" call site now shares:

- `rust/crates/host/src/flows/scan_all.rs` is now a thin `pub use lb_store::scan_all;` re-export.
  The flows slice keeps its module + import path (`super::scan_all::scan_all`), so the ten flows
  call sites are untouched, but the implementation they share is now the store-level one.
- `insight/notify.rs::load_subs` moved off its inline drain onto `scan_all` (its best-effort
  contract is preserved — a read error returns what was read so far; the reactor never fails a
  pass on a read hiccup).
- The seven host roster reads moved off the one-page `scan` onto `scan_all` (below).

### 2. The seven single-page roster reads → full drain

`rules/get.rs::rules_list`, `dashboard/store.rs::scan_dashboards`,
`panel/store.rs::scan_panels`, `report/store.rs::scan_reports`, `nav/store.rs::scan_navs`,
`brand/store.rs::scan_brands`, `render_templates/store.rs::scan_templates` — each was
`scan(store, ws, TABLE, MAX_*, None)` (one 200-row page) + filter-in-code; now
`scan_all(store, ws, TABLE)` + the same filter.

### 3. Removed the misleading `MAX_*` "caps"

`MAX_DASHBOARDS` / `MAX_PANELS` / `MAX_REPORTS` / `MAX_NAVS` / `MAX_BRANDS` / `MAX_TEMPLATES` were
each literally `= lb_store::MAX_SCAN_LIMIT` (200). They had **no external consumers** (verified by
grep across the repo — only their own `store.rs` referenced each) and `scan` clamps every request
to 200 server-side, so any value above 200 is silently 200. None was a genuine product bound —
every caller treats the result as the full set (`let all = scan_*(...)` then filters by
visibility/tombstone). Removed; a real roster cap is a separate product decision, to enforce
explicitly after a full drain if ever wanted.

## Decisions worth recording

- **Store-level seam, not a host-local one.** The issue offered "promote flows' scan_all to a
  shared host seam, or add a store-level drain — decide once". The store crate owns
  `scan`/`Page`/`Row`/`MAX_SCAN_LIMIT`, so the drain belongs there; both `lb-host` and `lb-insights`
  already depend on `lb_store`, so nothing new is wired. Two drains existed with drift
  (`host/flows/scan_all.rs` → `Vec<Row>`; `insights/table_scan.rs` → unwrapped `Vec<Value>` with a
  `MAX_ROWS` backstop) plus `notify.rs`'s inline loop; the canonical one collapses the first and the
  third.
- **No silent backstop.** A partial return at N would just relocate the "rows vanish past N" bug to
  a larger N — the exact class being fixed. `scan_all` drains to the end; the real bound is
  retention/config limits (the tables are bounded in practice). An unbounded table is a retention
  problem to solve separately, not something this read hides.
- **Full-drain-then-filter, not a prefix early-exit.** The scan cursor is the SurrealDB `<string>id`
  rendering (`⟨⟩`-bracketed for composite ids like `[series, producer, seq]`) whose ordering does
  NOT agree with the display id, so a cursor that "looks past" a prefix can still be ordered before
  a wanted row. Full drain is the only sound read. (A store-level prefix scan — cursor seeded at the
  prefix, server-side `string::starts_with` — is the named perf follow-up if a drain profiles hot.)
- **`insights/table_scan.rs` left alone.** It has a distinct, intentional bounded contract
  (`MAX_ROWS` = 10 000, partial return documented) and is already correct; forcing it through the
  unbounded `scan_all` would change its early-exit semantics. It is out of this sweep's scope.

## Testing

New `rust/crates/host/tests/roster_scan_paging_test.rs` (3 tests), each seeding past the 200-row
page boundary:

- `scan_all_drains_every_page_in_id_order` — pins the canonical seam itself: 250 rows in a scratch
  table come back, in id order, including the one a single page dropped.
- `dashboard_list_returns_target_past_one_scan_page` — a strict-decode roster with a visibility
  filter: 240 tombstoned `Dashboard` fillers (they decode but `list` skips them before gate 3) sort
  before the caller's own dashboard, which must still appear.
- `rules_list_returns_target_past_one_scan_page` — a loose-decode/authz roster: 240 junk fillers
  (swallowed by `SavedRule` decode) sort before a real rule, which must still appear.

The remaining CRUD stores (panel/report/nav/brand/render_templates) share the IDENTICAL
`scan_all` + envelope-unwrap shape as dashboard (read each); dashboard stands in for that class,
and the canonical-drain test pins the one function they all call.

All affected host suites green (real node, rule 9): dashboard 12, flows_scan_paging 4, insights 22,
nav 30, panel 10, render_templates 6, report 9, rules 22, plus the 3 new. `cargo check -p lb-store
-p lb-host` clean; `cargo fmt --check` clean.

## Notes for the next session

- Nothing committed — repo convention (the user commits/merges). Suggested branch:
  `feat/scan-paginate-non-flows-69`.
- `insight/notify.rs::load_subs` was already correct before this session; the change there is pure
  dedupe onto the shared seam (behavior identical, best-effort contract preserved).
- Optional perf follow-up (carried from the flows session): a store-level prefix scan would turn
  these full drains into O(prefix) reads if any roster ever profiles hot.
