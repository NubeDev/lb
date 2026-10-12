#!/usr/bin/env python3
"""Build the static, Latin-subset TTFs the PDF renderer embeds.

    python3 -m venv venv && ./venv/bin/pip install fonttools brotli
    ./venv/bin/python build_fonts.py <dir of google/fonts `ofl/` checkouts> <out dir>

Sources are the variable (or static) TTFs from github.com/google/fonts `ofl/<family>/`. Each is pinned
to Regular (400) and Bold (700) — Typst does not read variation axes, it needs one file per weight —
then subset to the Latin ranges below, which is what takes ~3 MB of fonts down to under 1 MB.
"""
import sys
from pathlib import Path

from fontTools import subset
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer

# (family dir, source file, output stem). A source with no `[` is already static.
FAMILIES = [
    ("inter", "Inter[opsz,wght].ttf", "Inter"),
    ("geist", "Geist[wght].ttf", "Geist"),
    ("ibmplexsans", "IBMPlexSans[wdth,wght].ttf", "IBMPlexSans"),
    ("sourceserif4", "SourceSerif4[opsz,wght].ttf", "SourceSerif4"),
    ("jetbrainsmono", "JetBrainsMono[wght].ttf", "JetBrainsMono"),
    ("ibmplexmono", "IBMPlexMono-{style}.ttf", "IBMPlexMono"),
]
WEIGHTS = [("Regular", 400), ("Bold", 700)]
# Basic Latin, Latin-1, Latin Extended-A, punctuation, currency, and the few symbols a report prints.
UNICODES = "U+0020-007E,U+00A0-017F,U+2010-2027,U+2030-203A,U+20A0-20BF,U+2122,U+2190-2193,U+2212,U+2264,U+2265"


def main(src: Path, out: Path) -> None:
    out.mkdir(parents=True, exist_ok=True)
    for folder, name, stem in FAMILIES:
        for style, weight in WEIGHTS:
            if "[" in name:
                font = TTFont(src / folder / name)
                axes = {a.axisTag: a.defaultValue for a in font["fvar"].axes}
                axes["wght"] = weight
                font = instancer.instantiateVariableFont(font, axes)
            else:
                font = TTFont(src / folder / name.format(style=style))
            # A pinned instance keeps the default master's style bits; the renderer picks Bold by
            # these, so state them rather than trust the source.
            bold = weight >= 700
            font["OS/2"].usWeightClass = weight
            font["OS/2"].fsSelection = (font["OS/2"].fsSelection & ~0x61) | (0x20 if bold else 0x40)
            font["head"].macStyle = (font["head"].macStyle & ~1) | (1 if bold else 0)
            opts = subset.Options()
            opts.layout_features = ["kern", "liga", "mark", "mkmk", "ccmp", "locl"]
            opts.name_IDs = [1, 2, 3, 4, 6, 13, 14, 16, 17]
            opts.notdef_outline = True
            sub = subset.Subsetter(opts)
            sub.populate(unicodes=subset.parse_unicodes(UNICODES))
            sub.subset(font)
            target = out / f"{stem}-{style}.ttf"
            font.save(target)
            print(f"{target.name:32} {target.stat().st_size:>8}")
        (out / f"{stem}-OFL.txt").write_bytes((src / folder / "OFL.txt").read_bytes())


if __name__ == "__main__":
    main(Path(sys.argv[1]), Path(sys.argv[2]))
