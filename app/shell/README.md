# App shell — the React Native host (iOS/Android)

**Status: scaffold only.** Scope: `docs/scope/app/app-shell-scope.md`.

The one RN host app: login → many workspaces (token per workspace, Keychain/Keystore),
REST + SSE to the gateway through the shared `@nube/app-sdk` client, `ext.list`
discovery → cap-gated nav → Module Federation mount of extension `Page`/`Widget`
components. Re.Pack 5 (Rspack) + Module Federation 2; `react`, `react-native`, and
`@nube/app-sdk` shared as singletons.

Nothing is generated yet — the implementing session initializes the RN project here
(pin RN/Re.Pack versions in its session doc) and follows `docs/FILE-LAYOUT.md` for the
feature tree (`src/features/session/`, `src/features/workspaces/`,
`src/features/ext-host/`, `src/features/channels/`, …), mirroring `ui/src` names so
the two shells read the same.
