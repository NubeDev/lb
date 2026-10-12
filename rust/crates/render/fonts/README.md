# Bundled brand fonts

The faces a brand may name beyond the three `typst-assets` ships. The renderer can only set type in a
font it carries — there is no system font on a node — so each family a brand editor offers has to be
here, compiled in by `src/fonts.rs`.

| Family (the name a brand stores) | Files | Licence |
| --- | --- | --- |
| Inter | `Inter-{Regular,Bold}.ttf` | `Inter-OFL.txt` |
| Geist | `Geist-{Regular,Bold}.ttf` | `Geist-OFL.txt` |
| IBM Plex Sans | `IBMPlexSans-{Regular,Bold}.ttf` | `IBMPlexSans-OFL.txt` |
| Source Serif 4 | `SourceSerif4-{Regular,Bold}.ttf` | `SourceSerif4-OFL.txt` |
| JetBrains Mono | `JetBrainsMono-{Regular,Bold}.ttf` | `JetBrainsMono-OFL.txt` |
| IBM Plex Mono | `IBMPlexMono-{Regular,Bold}.ttf` | `IBMPlexMono-OFL.txt` |

All six are SIL Open Font License 1.1, which permits bundling and subsetting; each licence file
travels with its font.

## What these files are

Not the upstream downloads. Each is **one static weight, subset to Latin**:

- **Static**, because Typst does not read variation axes. Upstream ships most of these as a single
  variable font; it is pinned to Regular (400) and Bold (700).
- **Regular and Bold only.** No italics: a brand's text in a report is the letterhead, cover and
  index, none of which italicise. Italic markdown in a flowed document falls back to the default
  serif's italic.
- **Latin subset** (Basic Latin, Latin-1, Latin Extended-A, punctuation, currency, a few symbols).
  That is the difference between ~3 MB and ~0.6 MB in the node binary. A character outside it
  falls through to the fallback face, the same way an unknown family does.

## Rebuilding

Sources are `ofl/<family>/` in <https://github.com/google/fonts>.

```sh
python3 -m venv venv && ./venv/bin/pip install fonttools brotli
./venv/bin/python build_fonts.py <path to google/fonts/ofl> .
```

Adding a family: a row in `build_fonts.py`, its two `include_bytes!` lines and its name in
`BRAND_FONT_FAMILIES` (`src/fonts.rs`). `tests/brand_fonts_test.rs` fails if a name is advertised
without a face behind it.
