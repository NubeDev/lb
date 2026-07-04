# App shell — the React Native host (iOS/Android)

**Status: shell slice SHIPPED** (RN 0.86.0 / React 19.2.3 / Re.Pack 5.2.5; standalone
install — see `.npmrc`). Scope: `docs/scope/app/app-shell-scope.md`; log:
`docs/sessions/app/app-shell-session.md`.

The one RN host app: login → many workspaces (token per workspace, Keychain/Keystore),
REST + SSE to the gateway through the shared `@nube/app-sdk` client, `ext.list`
discovery → cap-gated nav → Module Federation mount of extension `Page`/`Widget`
components. Re.Pack 5 (Rspack) + Module Federation 2; `react`, `react-native`, and
`@nube/app-sdk` shared as singletons.

Run it: `pnpm install` (standalone lockfile) then `pnpm start` (Re.Pack dev server) +
`pnpm android` / `pnpm ios`. Feature tree per `docs/FILE-LAYOUT.md`, mirroring `ui/src`
names: `src/features/{session,workspaces,channels,ext-host}/`, `src/lib/` (client +
node-url), `src/polyfills.ts` (streaming fetch — required for SSE; loads first).
Extension MOUNTING is not here yet — `ext.list` entries are listed only until the
app-extensions slice.
