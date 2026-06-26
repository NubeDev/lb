# doc-site/

The published documentation site, built with [Nextra](https://nextra.site/)
(Next.js + MDX).

## What this is — and isn't

- **Source of truth stays in `../docs/`.** Authored markdown lives there and
  follows the three-stage flow (`scope/` → `sessions/` → `public/`); see
  [`../docs/ABOUT-DOCS.md`](../docs/ABOUT-DOCS.md).
- **This site renders the *published* docs** — i.e. `../docs/public/` plus
  `../docs/vision/`. It is the reader-facing presentation layer, not a second
  place to author content.
- `scope/` and `sessions/` are working material and are **not** published by
  default (they can be exposed behind an "internals" section later if wanted).

So: write docs in `../docs/`, promote durable truth into `../docs/public/`, and the
site picks it up. Don't fork doc content into `doc-site/`.

Status: not yet scaffolded — architecture scope only. When set up, document the
build/dev commands here.
