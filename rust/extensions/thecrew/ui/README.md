# @nube/thecrew

The **graphics-canvas UI**: a three.js (`@react-three/fiber`) app for building AHU
plant graphics and floor plans from a symbol palette — proven as a standalone
playground, now being lifted into the thecrew extension (see
`../docs/thecrew-extension-scope.md`; parent feature scope:
`docs/scope/frontend/graphics-canvas-scope.md` at the repo root).

Since the move to `rust/extensions/thecrew/`, this app is **no longer a pnpm
workspace member** (extensions are self-contained, like `proof-panel/ui/` — own
install, own lockfile, built by the extension's `build.sh`).

## Start / dev

From inside `rust/extensions/thecrew/ui/`:

```sh
pnpm install
pnpm dev          # vite dev server, http://localhost:5173
```

Other scripts:

- `pnpm build` — production build (`vite build`)
- `pnpm typecheck` — `tsc --noEmit`
- `pnpm test` / `pnpm test:watch` — vitest

The playground vite `dev`/`build` still works standalone; the extension lift adds a
federation lib build (`dist/remoteEntry.js`) per the extension scope.

Start with **[`docs/README.md`](docs/README.md)** — the four scopes (master · look ·
builder UX · symbols) define what "done" means, including the screenshot test and the
60-second-AHU benchmark. `src/` is laid out per `thecrew-scope.md` §File layout; every
file states its one responsibility. The only fake in this package is
`src/data/simulator.ts` (declared in the scope — it is the seam the framework's
host-mediated bridge replaces).
