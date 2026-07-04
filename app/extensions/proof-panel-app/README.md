# proof-panel-app — mobile companion of the `proof-panel` extension

**Status: scaffold only.** Scope: `docs/scope/app/app-extensions-scope.md` (example 1).

The app-side bundle for the existing Tier-1 wasm extension
`rust/extensions/proof-panel` — the phone sibling of its `ui/` folder. It proves the
**backend-ext ↔ app-ext pairing** end to end, with no placeholders:

- `Page` — calls `proof-panel.proof.ping` and `proof-panel.proof.derive` through the
  scoped bridge and renders the real results (the mobile twin of the web page).
- `Widget` — renders the `proof.derived` series live via `bridge.watch`
  (gateway SSE), receiving v3 frames in `ctx.data`.

There is **no manifest here**: the single source of truth stays
`rust/extensions/proof-panel/extension.toml`, which gains the additive block

```toml
[app]
entry = "app/dist/remote.container.js.bundle"
scope = ["proof-panel.*", "series.latest", "series.watch"]
```

pointing at this folder's build output. Build: a `build.sh` (Re.Pack 5 + Module
Federation 2, JS-only — the build fails if a native module outside the shell's shared
set enters the graph), invoked from the extension's existing `build.sh`.

Planned layout (per `docs/FILE-LAYOUT.md`):

```
src/
  remote.ts            ← MF container entry: exposes { Page, Widget } (AppRemote)
  Page.tsx             ← the page component
  ProofWidget.tsx      ← the derived-series widget
  useDerived.ts        ← bridge-backed data hook (call + watch)
build.sh
```
