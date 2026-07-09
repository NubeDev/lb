# doc-site/

The published documentation site, built with [Nextra 4](https://nextra.site/)
(Next.js App Router + MDX).

## What this is — and isn't

- **The shipped docs live here.** `content/public/` is the real, authored home for the *published*
  documentation (as MDX) — not a symlink, not a fork. It was promoted out of `docs/` so there is exactly
  one source of truth for what shipped.
- `content/vision/` is still a symlink into [`../docs/vision/`](../docs/vision/) (the north star).
- `scope/`, `sessions/`, and `debugging/` stay in [`../docs/`](../docs/ABOUT-DOCS.md) and are **not**
  published — they're working material.

So: the public/shipped docs are authored and edited directly under `doc-site/content/public/`; `vision/`
is pulled from `docs/`.

## Content wiring

Nextra 4 reads from `content/`:

```
content/
├── index.mdx      ← site landing page (authored here — it's chrome, not a doc)
├── _meta.js       ← top-level nav ordering (Home / Docs / Vision)
├── public/        ← the shipped docs, authored here as .mdx (was docs/public/)
└── vision  → ../../docs/vision    (symlink — the north star)
```

Public pages are **MDX** (`.mdx`). Use Markdown (`.md`) only for the rare page that must stay plain. When
converting prose to MDX, mind that raw `<tag>`, `{expr}`, and `}` in prose are parsed as JSX — escape them
(backticks for inline code, or `\`-escape) or keep them inside fenced/inline code spans. Multi-line inline
code spans that interleave other backticks on the same line can mis-close; keep brace-heavy code spans on a
single line.

> Route casing is preserved from the source filename. A file `content/public/SCOPE.mdx`
> is served at `/public/SCOPE`, not `/public/scope`. Link with the exact casing.

## Prerequisites

- Node.js 18+ (developed on Node 22)
- [pnpm](https://pnpm.io) (the repo's package manager)

## Commands

```sh
pnpm install        # install dependencies

pnpm dev            # dev server with HMR → http://localhost:3010

pnpm build          # static HTML export → ./out/ (no Node server at runtime)
pnpm start          # serve the production build locally on :3010
```

`pnpm build` runs `next build` (static export via `output: 'export'`) followed by
`postbuild`, which generates the [Pagefind](https://pagefind.org) client-side
search index under `out/_pagefind` and adds `out/.nojekyll`.

## Deploying (static host)

The build output in `out/` is a fully static site — host it on GitHub Pages,
Netlify, Cloudflare Pages, S3, etc. For a sub-path deploy (e.g. a GitHub Pages
project site at `/lb`), set `DOCS_BASE_PATH=/lb` at build time:

```sh
DOCS_BASE_PATH=/lb DOCS_REPO_URL=https://github.com/NubeIO/lb pnpm build
```

`DOCS_BASE_PATH` prefixes all routes/assets; `DOCS_REPO_URL` points the navbar
and "Edit this page" links at the repository.

## Layout

```
doc-site/
├── app/
│   ├── layout.jsx                # root layout: Nextra theme <Layout>/<Navbar>/<Footer>
│   ├── globals.css               # small theme overrides
│   └── [[...mdxPath]]/page.jsx   # catch-all that renders every MDX/MD page
├── content/                      # public/ (real .mdx) + vision symlink + landing page
├── mdx-components.js             # wires the docs theme's MDX components
├── next.config.mjs               # nextra() + static export + base path
├── tsconfig.json
└── package.json
```
