# doc-site/

The published documentation site, built with [Nextra 4](https://nextra.site/)
(Next.js App Router + MDX).

## What this is — and isn't

- **Source of truth stays in `../docs/`.** Authored markdown lives there and
  follows the three-stage flow (`scope/` → `sessions/` → `public/`); see
  [`../docs/ABOUT-DOCS.md`](../docs/ABOUT-DOCS.md).
- **This site renders the *published* docs** — i.e. `../docs/public/` plus
  `../docs/vision/`. It is the reader-facing presentation layer, not a second
  place to author content.
- `scope/` and `sessions/` are working material and are **not** published
  (only `public/` and `vision/` are wired in — see "Content wiring" below).

So: write docs in `../docs/`, promote durable truth into `../docs/public/`, and
the site picks it up. Don't fork doc content into `doc-site/`.

## Content wiring

Nextra 4 reads from `content/`. We do **not** copy or fork the docs — `content/`
contains only symlinks into the real source plus the site's own landing page:

```
content/
├── index.mdx      ← site landing page (authored here — it's chrome, not a doc)
├── _meta.js       ← top-level nav ordering (Home / Docs / Vision)
├── public   → ../../docs/public    (symlink — the shipped docs)
└── vision   → ../../docs/vision    (symlink — the north star)
```

Because `scope/`, `sessions/`, and `debugging/` are never symlinked, they can
never leak onto the site. Markdown (`.md`) is rendered as-is — **no MDX
conversion needed**. Use `.mdx` only if a page needs embedded React components.

> Route casing is preserved from the source filename. A file `docs/public/SCOPE.md`
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
├── content/                      # symlinks into ../docs + landing page (see above)
├── mdx-components.js             # wires the docs theme's MDX components
├── next.config.mjs               # nextra() + static export + base path
├── tsconfig.json
└── package.json
```
