# Report letterhead + cover switch — session

**Date:** 2026-10-12 · **Branch:** `feat/report-letterhead` · **Extends:**
`docs/scope/reports/report-pagination-and-export-options-scope.md` (the `ExportOptions` contract).

## The ask

A report PDF always opened on a near-empty cover page, and there was no way to put a letterhead on
the body pages. Two findings on the way in:

- `report_export` resolved its brand with a hard-coded empty id, so it always got
  `Brand::default()`. A brand saved through `brand.save` could not reach a report at all.
- The brand's `header_text`/`footer_text` doc comments promise `{page}`/`{title}`/`{date}` tokens.
  Nothing substitutes them; they print as typed. Left alone here (the letterhead replaces the need),
  recorded so it is not rediscovered.

- A brand's `logo_asset_id` may be an inline `data:` URI (the shell's brand editor stores small logos
  that way). `report_export` only ever looked it up as an asset, so an uploaded logo printed nothing,
  on the cover or anywhere. Fixed here (`inline_logo` in `report/export.rs`), since a letterhead whose
  `${__logo}` is always empty is not a feature.

## What changed

Three additive fields on `ExportOptions` (`report/options.rs`). Absent ⇒ the shipped document, byte
for byte, pinned by `absent_options_and_the_spelled_out_defaults_are_byte_identical`.

| Field | Meaning | Absent |
| --- | --- | --- |
| `cover: Option<bool>` | `false` drops the cover page | cover printed, as before |
| `brandId: String` | which brand supplies logo/colours/fonts | neutral default, as before |
| `letterhead: { topLeft, topRight, bottomLeft, bottomRight }` | text per corner of every body page | the brand's running header/footer |

`cover` is an `Option` because a bare `bool` defaults to `false`, which would have removed the cover
from every existing caller. In the renderer the same fact is spelled `RenderOptions::no_cover`.

### Who resolves what

The CALLER resolves its own variables before the export call (a dashboard client already owns that
grammar and the live scope). The renderer fills only the three references nobody else can:

| Token | Why only the renderer can |
| --- | --- |
| `${__logo}` | it holds the brand's logo bytes |
| `${__report.page}` | known at layout time |
| `${__report.pages}` | known at layout time |

Anything else in a corner is emitted as a Typst **string literal** from code mode
(`render/src/letterhead.rs`), never as markup — a corner reading `#line` prints `#line`.
`a_letterhead_with_every_token_and_hostile_text_compiles` pins it.

### Decisions

- **A letterhead owns both bands.** With one set, the brand's header/footer text and the
  `pageNumbers` counter are not also drawn; a page number is `${__report.page}` in a corner. Two
  sources for one band is two things that can disagree.
- **Four blank corners are no letterhead** (`ExportOptions::letterhead` returns `None`), so clearing
  every box restores the brand bands rather than erasing them.
- **`header-ascent` drops 40% → 20% when a letterhead is set.** Three lines plus a logo otherwise
  print against the paper's top edge. No letterhead ⇒ 40%, unchanged.
- **The letterhead names the brand's body font explicitly** (`BandStyle`), plus a lightened brand
  text colour. It cannot inherit it: a band takes its text style from the page it decorates, and a
  report page is placed images with no flowed text, so the document's `set text(font: ..)` never
  reached the band and it printed in the fallback face whatever the brand said.
  `the_letterhead_is_set_in_the_brands_body_font` pins it. **The brand's own running header/footer
  has the same flaw on report pages and is NOT fixed here** — fixing it changes the bytes of every
  existing export that sets header/footer text.
- **Page numbers still count the cover** when one is printed — existing `pageNumbers` behaviour, not
  touched.

## Brand fonts — six bundled families

A brand could only name the three faces `typst-assets` ships, so a workspace themed in Inter printed
its reports in a serif. `render/fonts/` now bundles **Inter, Geist, IBM Plex Sans, Source Serif 4,
JetBrains Mono and IBM Plex Mono** (the shell theme's self-hosted set), Regular + Bold, and
`render/src/fonts.rs` exports `BRAND_FONT_FAMILIES` — the closed list a brand editor should offer.

- **~0.6 MB added to the node binary, not ~3 MB.** Each file is one static weight (Typst does not
  read variation axes) subset to Latin. `render/fonts/README.md` says what the files are, their
  licence (all OFL 1.1, licence files alongside) and how to rebuild them (`build_fonts.py`).
- **No italics, Latin only.** Anything outside falls through to the fallback face, as an unknown
  family does.
- `render/tests/brand_fonts_test.rs` fails if a family is advertised without a real Regular and Bold.

## A logo of last resort — `options.logo`

A fourth additive field: an inline `data:image/...;base64,...` URI drawn **only when the brand
supplies no logo** (no brand picked, a brand without one, or one unreadable under the grant). It
exists because a shell may keep its workspace logo somewhere this host does not read (rubix-ai keeps
it on a prefs axis), and without it a letterhead is logo-less until someone re-uploads the same image
into a brand. A brand's own logo always wins.

Validated loudly (`report/logo.rs`): set but not a decodable image `data:` URI, or over 1 MiB
decoded, is a `BadInput` naming `options.logo` — checked before the record read, like `paper`.

## Tests

- `render/src/letterhead.rs` — the corner grammar (literals, line breaks, the three tokens, an
  unknown reference printing as typed, an empty band being `none`).
- `render/tests/letterhead_test.rs` — real compiles: `no_cover` removes exactly one page; a
  letterhead with every token and Typst-hostile text compiles; a logo token with no logo compiles;
  the letterhead is set in the brand's body font.
- `host/src/report/options.rs` — the wire shape and each field's absent state.
- `host/tests/report_export_letterhead_test.rs` — real store + real compile: `cover: false` drops one
  page and `cover: true` is byte-identical to absent; a letterhead changes the document without
  changing its page count; `brandId` selects the brand and an unknown id is the neutral default; an
  inline `data:` URI logo is drawn; the fallback logo fills in and never overrides; a bad or
  oversized fallback is a named `BadInput`.
- `host/src/report/logo.rs` — the `data:` URI decode and the fallback's validation.
- `render/tests/brand_fonts_test.rs` — every advertised family prints in its own face, both weights.

## Not done

- No per-line styling or centre slot in a band.
- The dead `{page}`/`{title}`/`{date}` promise on brand header/footer text (above).
