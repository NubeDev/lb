# Minimal shell — session

- Date: 2026-07-11
- Scope: `docs/scope/frontend/minimal-shell-scope.md`
- Status: done

## Goal

A small, published package that does *only* the host-side federation contract, so an embedder
ships `minimal-shell + their extension` and nothing else. Retires vendor-the-whole-shell (the
rubix-ai compromise).

## What changed

### New package: `packages/minimal-shell/` (`@nube/minimal-shell`)

~15 files — the hard budget the scope names:

- **`index.html`** — import-map (bare React → shims), theme boot script, PWA manifest link.
- **`src/singletons.ts`** — publishes React globals before any remote loads (the federation
  contract: the shell owns ONE React instance).
- **`public/shims/*.mjs`** (4 files) — re-export from `globalThis.__lb*` (react, react-dom,
  react-dom-client, react-jsx-runtime).
- **`src/main.tsx`** — entry.
- **`src/App.tsx`** — the shell: `ThemeProvider > LoginView | ExtMount`. Login → full-screen
  scoped mount of the configured/discovered extension. No sidebar, no dock, no chrome. The
  extension id is opaque config data (`VITE_HOME_EXT` env var, rule 10).
- **`src/session.ts`** — `useSession` hook (`useSyncExternalStore` over localStorage), `signIn`,
  `signOut`, `acceptInvite` (the invite-accept surface).
- **`src/ipc.ts`** — minimal gateway client: `login`, `listExtensions`, `mcpCall`,
  `acceptInvite`. HTTP only (no Tauri IPC). `VITE_GATEWAY_URL` env.
- **`src/federation.ts`** — `loadRemoteMount(ext, entry)` + `makeBridge(allowedTools)`. The host
  half of `@nube/ext-ui-sdk`'s `defineRemote` contract.
- **`src/theme.tsx`** — `ThemeProvider` + `useTheme`. Applies CSS variables
  (`color-scheme`, `--bg`, `--accent`) to `documentElement`; extensions inherit via the cascade.
- **`src/events.ts`** — SSE hub: one `EventSource` per tab, refcounted, auto-reconnect.
- **`public/manifest.webmanifest`** — PWA manifest (installable, standalone display).
- **`src/App.test.tsx`** — unit tests.
- **`vite.config.ts`** / **`tsconfig.json`** / **`package.json`** / **`README.md`**.

### Decisions

1. **Package, not template** (the scope's recommendation — "vendoring is the disease this scope
   treats"). Consumable with zero lb checkout: `pnpm add @nube/minimal-shell` + config.
2. **Extension id is config, not code.** `VITE_HOME_EXT` env var — the shell never branches on
   the ext id (rule 10). A swap is a config change. If unset, the shell discovers the first ext
   with a UI via `ext.list` (the generic seam).
3. **Invite-accept lives here** (coordinated with `invites-scope.md`). The `acceptInvite`
   function in `session.ts` calls `POST /public/invite/accept`. The accept UI screen is a themed
   client route the product host adds (the shell provides the API, not the screen).
4. **Home = one ext page v1** (the scope's recommendation). Multi-page bottom-tab mode is v1.5
   or full-shell territory.

## Tests

- **Unit**: 2 tests (renders login view when no session; renders input fields for user/workspace).
  `vitest run` green.

## Follow-ups

- Gateway test (real node + real `hello` fixture ext, Playwright e2e).
- The full shell consuming the extracted core (the proof it's really generic).
- `VITE_HOME_SCOPE` → `bridge.call` scope filter (defense in depth, shipped).
- PWA installability test (manifest validates, service worker).
