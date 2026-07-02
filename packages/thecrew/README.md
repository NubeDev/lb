# @nube/thecrew

The **graphics-canvas UI/UX test bed**: a standalone three.js (`@react-three/fiber`)
playground for building AHU plant graphics and floor plans from a symbol palette —
100% focused on how it **looks** and how the **builder feels**. What it proves lifts
into the `graphics-canvas` extension (see `docs/scope/frontend/graphics-canvas-scope.md`
at the repo root); what it disproves dies cheaply here.

## Start / dev

```sh
pnpm install                          # from the repo root (workspace member)
pnpm --filter @nube/thecrew dev       # vite dev server, http://localhost:5173
```

Or from inside `packages/thecrew/`:

```sh
pnpm install
pnpm dev
```

Other scripts (run the same way, from the repo root with `--filter @nube/thecrew`
or locally with just the bare script name):

- `pnpm build` — production build (`vite build`)
- `pnpm typecheck` — `tsc --noEmit`
- `pnpm test` / `pnpm test:watch` — vitest

This is a standalone playground, not a library — there's nothing to publish or link;
just run `dev` and edit `src/`.

Start with **[`docs/README.md`](docs/README.md)** — the four scopes (master · look ·
builder UX · symbols) define what "done" means, including the screenshot test and the
60-second-AHU benchmark. `src/` is laid out per `thecrew-scope.md` §File layout; every
file states its one responsibility. The only fake in this package is
`src/data/simulator.ts` (declared in the scope — it is the seam the framework's
host-mediated bridge replaces).
