# `app/` — the React Native mobile app (iOS/Android)

The mobile twin of the React shell: **one host app** (`app/shell/`) that is a thin,
capability-scoped client of the gateway, plus **federated extensions** — the same
"everything else is an extension" doctrine as the web UI, delivered to the phone with
Re.Pack + Module Federation instead of the browser's ESM `import(url)`.

Read order (each scope is one ask; build them in this order):

1. **`app-shell-scope.md`** — the RN host: login, multi-workspace session, the
   transport decision (REST + SSE over the gateway; zenoh-ts rejected — see the doc),
   extension discovery via `ext.list`, and navigation.
2. **`app-extensions-scope.md`** — the mobile extension model: the additive `[app]`
   manifest block, Re.Pack/MF2 remotes, the **JS-only rule**, the component-based
   mount contract, signing/publishing through the unchanged `Artifact` path, and the
   two reference extensions (`proof-panel-app`, `channel-chat`).
3. **`app-sdk-scope.md`** — `@nube/app-sdk` (`app/sdk/`): the contract package every
   app extension builds against, and the long-term **shared panel/widget SDK** story
   with the dashboard (`ui/src/lib/panel-kit/`, the v3 frames-in `WidgetCtx`).
4. **`app-expo-scope.md`** — adopt Expo's native module library (`expo-*`, bare
   install) and keep EAS Build available **without** losing Re.Pack + Module
   Federation; rejects the Metro-coupled managed workflow. Additive to the shell.

Source tree this topic owns (workshop layout, see each scope):

```
app/
  shell/         ← the RN host app (Re.Pack, one per platform build)
  sdk/           ← @nube/app-sdk — contract types + bridge/client bindings
  extensions/    ← app-side extension bundles (JS-only MF remotes)
    proof-panel-app/   ← companion page+widget for the existing wasm proof-panel ext
    channel-chat/      ← pure-app ext: channels + in-channel AI agent
  docs/          ← pointer only; authored docs live here in docs/scope/app/
```

Related: `../extensions/extensions-scope.md` (the extension model this reuses),
`../extensions/ui-federation-scope.md` (the web counterpart), `../frontend/dashboard/`
(the widget contract the shared SDK aligns with), `../auth-caps/global-identity-scope.md`
(multi-workspace login), `../channels/` (the channel surface the app consumes).
