# Single scan page drops rows past 200 — rosters vanish outside flows (rules/dashboard/panel/report/nav/brand/render_templates)

- Area: store (cross-cutting — the host roster reads)
- Status: resolved
- First seen: 2026-07-15 (filed as the follow-up to the flows read-back hardening,
  [#69](https://github.com/NubeDev/lb/issues/69); latent since each roster verb shipped — only
  reproduces once a workspace's config table outgrows one scan page, so fresh suites stayed green)
- Resolved: 2026-07-15
- Session: ../../sessions/store/roster-scan-paging-session.md
- Regression test: rust/crates/host/tests/roster_scan_paging_test.rs
  (`scan_all_drains_every_page_in_id_order`,
  `dashboard_list_returns_target_past_one_scan_page`,
  `rules_list_returns_target_past_one_scan_page`)
- Sibling (the same bug class in flows): [flows/single-scan-page-drops-rows-past-200.md](../flows/single-scan-page-drops-rows-past-200.md)

## Symptom

On a long-lived node, a roster list silently returns a PARTIAL set once the workspace holds more
than 200 rows of that config: `dashboard.list` / `panel.list` / `report.list` / `nav.list` /
`brand.list` / `template.list` / `rules.list` each return only the first 200 (id-ordered) and drop
the rest — no error, no truncation flag. A workspace with 250 dashboards lists 200; the 50
last-sorting ones are invisible until enough earlier ones are deleted. `insight.notify`'s sub
loader was the one listed site already draining correctly.

## Reproduce

Seed 200+ rows into one of the config tables whose ids sort before a target the caller owns, then
call the `.list` verb: with the bug, the target sorts past page 1 and is absent from the roster;
after the fix, every row drains and the target appears. See the regression test.

## Investigation

Same root cause as the flows read-back bug, one layer over: every roster read called
`lb_store::scan(…, MAX_SCAN_LIMIT, None)` **once** and filtered in code, never following `page.next`.
Seven host call sites had the pattern — `rules/get.rs::rules_list`,
`dashboard/store.rs::scan_dashboards`, `panel/store.rs::scan_panels`, `report/store.rs::scan_reports`,
`nav/store.rs::scan_navs`, `brand/store.rs::scan_brands`, `render_templates/store.rs::scan_templates`.
`insight/notify.rs::load_subs` already had a correct inline cursor loop (the only listed site that
did). The caps looked intentional (`MAX_DASHBOARDS`, `MAX_PANELS`, …) but each was literally
`lb_store::MAX_SCAN_LIMIT` (200) and `scan` **clamps every request to 200 server-side** — so any cap
above 200 is silently 200, and a single-page read is silently the whole table only while the table
fits one page.

## Root cause

`lb_store::scan` is deliberately one bounded page with a cursor (the DB-browser grid contract); the
roster verbs treated one page as "the whole table", and the `MAX_*` "caps" were the page size
mislabeled as a bound. Below 200 rows the two are identical — every test and a young deployment
passes — and past 200 the later-sorting records silently vanish from the roster.

## Fix

Promoted the cursor loop to ONE canonical, cross-crate seam — `lb_store::scan_all`
(`rust/crates/store/src/scan_all.rs`) — that drains every page of a ws table in id order and returns
the raw rows. Every roster read now goes through it:

- `rules/get.rs`, `dashboard/store.rs`, `panel/store.rs`, `report/store.rs`, `nav/store.rs`,
  `brand/store.rs`, `render_templates/store.rs` — swapped the one-page `scan` for `scan_all`.
- `insight/notify.rs::load_subs` — moved off its inline loop onto `scan_all` (best-effort contract
  preserved: a read error returns what was read so far, the reactor never fails a pass on a hiccup).
- `host/flows/scan_all.rs` — now a thin re-export of `lb_store::scan_all`, so the flows slice and the
  roster reads share the ONE implementation (the ten flows call sites keep their import path).

The misleading `MAX_DASHBOARDS` / `MAX_PANELS` / `MAX_REPORTS` / `MAX_NAVS` / `MAX_BRANDS` /
`MAX_TEMPLATES` constants were removed — they were `= MAX_SCAN_LIMIT` aliases with no external
consumers (verified), and leaving them would re-mislabel the page size as a bound. None of these
rosters had a genuine product cap (every caller treats the result as the full set: `let all =
scan_*(...)` then filters by visibility/tombstone); a real cap is a separate product decision, to
enforce explicitly after a full drain if ever wanted.

Full-drain-then-filter on purpose, with NO silent backstop: a partial return would just relocate
the "rows vanish past N" bug to a larger N. The scan cursor is the SurrealDB `<string>id` rendering
(`⟨⟩`-bracketed for composite ids) whose ordering disagrees with the display id, so a prefix-seeded
early exit is unsound. `rust/crates/insights/src/table_scan.rs` keeps its own bounded
`MAX_ROWS`-backstopped drain by design (a distinct, intentional contract — partial return at 10 000
is documented there); it is out of this sweep's scope.

## Lesson / prevention

A paged API used as if it were a full read is invisible until production data outgrows one page —
tests must seed **past the page boundary** when the code filters a shared table in memory. And a
"cap" that is literally the page size is not a cap, it is the bug wearing a constant's clothes:
`scan` clamps every request to 200, so a `MAX_FOO` above 200 is silently 200. When a roster needs a
real bound, enforce it explicitly after a full drain (or via a genuine server-side limit) — never
inherit the page clamp as an accidental bound.
