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

## Expo: bare modules only — Re.Pack is the one bundler (DO NOT run `expo prebuild`/`expo start`)

The shell adopts Expo's **bare module system** (`expo-modules-core` + `expo-*`, pinned to
**SDK 57** — the SDK that pairs with RN 0.86) so it can use native modules like
`expo-secure-store` (the session-token store). It does **not** adopt the managed workflow.
This posture is load-bearing:

- **Re.Pack (Rspack) + Module Federation stay the only bundler.** Never add `expo start`,
  Expo Go, expo-router, or EAS Update — they are Metro-coupled and would displace Re.Pack,
  which is the whole reason extensions work. The native wiring (`android/settings.gradle`,
  `MainApplication.kt`, `ios/Podfile`, `AppDelegate.swift`) links expo modules but leaves
  every JS-bundling touchpoint on Re.Pack (moduleName `LazybonesShell`, debug bundle root
  `index`).
- **Do not run `expo prebuild`** against this tree — it would overwrite the hand-owned
  native config with managed (Metro) defaults. Any Metro config file (`metro.config.js`,
  `.expo/…`) appearing in the tree is a regression, not a fixture.
- **Standalone install policy** lives in `pnpm-workspace.yaml` (this project's own, not the
  repo root's): `minimumReleaseAge: 0` (SDK-57 packages are freshly published) and
  `allowBuilds`. It is not a workspace-member list — keep `packages:` out of it or the
  React 18/19 split (`.npmrc`) re-breaks.
- **RN upgrades now also check the Expo SDK ↔ RN matrix** — one line on the existing
  RN/Re.Pack upgrade checklist. Bump the Expo SDK deliberately, pinned.

Scope: `docs/scope/app/app-expo-scope.md`; log: `docs/sessions/app/app-expo-session.md`.
