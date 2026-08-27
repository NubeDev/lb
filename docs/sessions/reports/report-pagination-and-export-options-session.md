# Reports — authored page breaks + a real export-options contract (session)

- Date: 2026-08-27
- Scope: ../../scope/reports/report-pagination-and-export-options-scope.md
- Status: done (implemented + tested on `feat/report-pagination-and-export-options`; **not tagged**)
- Consumer: rubix-ai `docs/scope/frontend/reports/report-builder-ux-scope.md` Phase 1+, which is
  blocked until this ships as a `node-v*` tag.

## Goal

Two additions, both additive and serde-defaulted:

1. **`Cell.pageBreakBefore`** — an author-set page break, honoured by `paginate` ahead of its fit
   rule, so a report author can say what goes on a page instead of dragging panels until the row
   arithmetic happens to agree.
2. **`ExportOptions`** on the export contract — paper, orientation, margins, scale, page numbers,
   index. `report.export` accepted `{ snapshots }` and nothing else, and the renderer's
   `RenderOptions { page_numbers, index }` had been implemented for months with **nothing setting
   it** — two finished features dark behind one unplumbed line.

Plus the refactor both need: the six hardcoded `A4_*` constants become one `PageGeometry` value
threaded through placement, pagination and the Typst template.

## What changed, in the order the scope demanded

The scope calls the geometry parameterisation "the risky part, not the feature" and prescribes the
order: land it with A4 as the only value and the round-trip test green, *then* build on it. That is
what happened, and each step was green before the next began.

**1 — `PageGeometry`, a pure refactor.** `geometry.rs` now owns a value type (`w_mm`, `h_mm`, the
three margins, `scale`) with `a4_portrait()` as `Default` and everything derived — content box,
pixel widths, `rows_per_page` — as methods. The `A4_*` consts stay: they are what *defines* the
default and what `a4-sheet.ts` mirrors, so the default cannot drift from the screen without the
round-trip test noticing. The free functions (`a4_rows_per_page()` and friends) remain as
A4-portrait shorthands, so every shipped caller kept compiling.

`geometry.rs` was 346 lines and this would have pushed it past the FILE-LAYOUT ceiling, so the
placement half moved to a new **`cell_rect.rs`** (`RectMm` + `cell_rect_mm*`) — the same split
`paginate.rs` already got, and for the same reason: three questions (which paper, which page, where
on it), three files, one shared page box.

> Three tests were dropped in the rewrite and restored before moving on
> (`a_tall_band_that_would_be_squashed_flows_to_the_next_page_at_full_height`,
> `a_band_taller_than_a_whole_page_still_gets_its_own_page_and_is_clamped`,
> `a_board_that_already_fits_is_paginated_exactly_as_before`) — they assert a placement under a
> pagination, so they live in `cell_rect.rs` now. Caught by diffing the test-function names against
> `HEAD`, not by noticing.

**2 — the marker.** `Cell.page_break_before` follows the shipped byte-stable bool pattern exactly
(`serde(default, deserialize_with = "null_default", rename = "pageBreakBefore", skip_serializing_if
= "is_false")`, copied from `transparent`), so a pre-feature cell serialises unchanged.

`paginate` becomes `paginate_with(&PageGeometry, &[Band])` where `Band { y, h, break_before }`;
`paginate(&[(y, h)])` stays as the A4-no-markers shorthand, which is what makes "every shipped
paginate test passes unchanged" literally true rather than approximately. The new clause sits
**ahead** of the fit test and is guarded by `row_on_page > 0`, which is what stops a marked band
that already starts a page from emitting a gratuitous blank one. Markers collapse across a row with
**OR**, so marking any tile of a KPI row breaks before the whole row — anything else would tear a
band, which the collapse exists to prevent.

**3 — the trap the scope flagged, and it was real.** `paginate` never sees `Cell`: `compose_pages`
lays out over `Placed`, built from the client's rendered panels, and a `RenderedPanel` carries no
marker because the marker is a property of the *record*, not of the photograph. `Placed` gains
`break_before` and `panels_to_place` resolves it by id.

While doing that I pulled the id resolution out of `title_for` into one `cell_for` that both use.
Two copies of "which cell is this" is exactly the pair that drifts, and the drift would have been
silent in the worst way: a repeat clone keeping its title but losing its page break.

**4 — `ExportOptions`** (`report/options.rs`), taken by **both doors** — the HTTP route's
`ExportBody` and the `report.export` MCP arm — as the same type, deliberately. It resolves to a
`PageGeometry` and a `RenderOptions`; `report_export` now sets `assembled.options`, which is the
line that turns page numbers and the index from dead code into features. Validation is loud: an
unknown `paper`/`orientation`, a negative margin, a non-finite scale, or margins that swallow the
page are each a `BadInput` **naming the field**, resolved *before* the record is read so a typo
costs no dashboard read, brand resolve or Typst compile.

`pdf.rs` emits `#set page` from the geometry in explicit millimetres rather than `paper: "a4"`, and
`place.rs`'s canvas is the geometry's content box — a mismatch there would put every rect at the
right millimetre of the wrong box.

## The backward-compatibility guarantee, actually verified

The scope asks for "options absent ⇒ **byte-identical** PDF to the current implementation", and says
it must be a byte assertion rather than a smoke test. A test inside the branch can only compare the
branch against itself, so I checked it against the real thing: built a fixed report (including a
band at row 26 — the flow case), exported it on this branch, `git stash`ed, exported the same board
on `master`, and compared.

```
$ sha256sum old.pdf new.pdf
18ce98285325a6d2fa240e4a93a71368fa212ac6e711ca664798d98ff08a2f8d  old.pdf   # master
18ce98285325a6d2fa240e4a93a71368fa212ac6e711ca664798d98ff08a2f8d  new.pdf   # this branch
$ cmp old.pdf new.pdf && echo "BYTE IDENTICAL"
BYTE IDENTICAL
```

Identical **despite** `#set page` changing from `paper: "a4", margin: (x: 2.2cm, …)` to explicit mm
— Typst resolves both to the same page. That is the claim the scope wanted and it is now a measured
fact rather than a design intention. The in-repo test
(`absent_options_and_the_spelled_out_defaults_are_byte_identical`) pins the durable half: absent
options and the fully spelled-out defaults must agree, and the media box must be A4.

## Tests

Every category the scope's testing plan names:

- **`paginate`** — marked band starts a page though it fitted; a marked band already at a page top
  adds no blank page; the marker never compacts a deliberate blank page; marker + over-tall band
  starts its page and stops; a marker on any cell of a row breaks the whole row; a shorter page
  paginates the same board into more pages. All four shipped tests pass **unchanged**.
- **`geometry`/`cell_rect`** — the round-trip test pins the same numbers as before; the default is
  A4 to the millimetre across the value type and the shorthands; landscape swaps paper but not
  margins and round-trips; aspect-ratio and uniform-reduction properties now assert across *every*
  geometry, not just A4.
- **`compose_pages`** — a marked cell starts its own page **and is measured from that page's top**
  (the regression that drew a flowed panel at the offset it had on the page it left), with the
  unmarked control proving the fixture is not doing the work; a shorter page composes more pages.
- **`panels_to_place`** — the trap: a marker set on a cell is honoured when the layout comes from
  client-rendered panels, a repeat clone inherits its source's marker, and the record-driven
  fallback reads it off the cell.
- **`report_export`** — byte-identical defaults; `pageNumbers`/`index` each change the output (and
  the index *adds* a page); Letter and landscape reach the actual media box; an unknown paper is a
  named `BadInput` raised before the record read.
- **Both doors** — the shipped parity test now also pushes a *non-default* profile (different paper,
  different orientation, page numbers on) through both and asserts byte equality, plus that the
  options changed something. Parity on defaults alone would still pass if one door ignored options.
- **Capability-deny** — the shipped `report.export` deny tests pass unchanged, both ways, proving no
  new gate slipped in. No new cap was added.

```
lb-render:                        49 passed  (+ 3 placed_page)
lb-host  --lib report::           34 passed
report_export_test.rs              3 passed
report_export_media_test.rs        8 passed
report_export_options_test.rs      5 passed
```

## A pre-existing breakage repaired in passing

`cargo test -p lb-host --lib` **did not compile on `master`**: `rust/crates/host/src/dashboard/
grafana/bind.rs`'s test fixture builds a `Target` literal and `Target::show_when` had been added
without updating it. Confirmed pre-existing by stashing this branch's work and reproducing on a
clean tree. It blocks every unit test in the crate — including the ones written here — so it is
fixed (one field, `show_when: String::new()`, commented as such). Unrelated to this scope; called
out so it is not read as part of it.

Also noted, **not** fixed: `cargo test` across the workspace fails to build `lb-cli`'s
`ext_publish_test` because a `hello-v2` wasm artifact is missing from the tree. Pre-existing, out of
scope here.

## Follow-ups

- **This is not tagged.** The change is committed on `feat/report-pagination-and-export-options` and
  nothing is released. Publishing a `node-v*` tag is a release decision, not a coding one — it is
  what unblocks rubix-ai Phase 1 (`WORKFLOW-LB.md` §4: PR → tag → bump), and it needs a human to
  say go.
- **`rows_per_page` is no longer a constant**, which the scope names as the seam where screen and
  print can silently diverge again. The client currently hardcodes `28`. rubix-ai's
  `reportPagination.ts` must take the geometry from the same profile the export will use — including
  the scheduled one — rather than re-deriving it. That obligation belongs to the consumer scope and
  is written down there.
- The public docs promotion happens when the pair ships, not per-repo.
