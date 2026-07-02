# Federated page won't mount: `remote does not export a \`mount\` function` (export was `mountPage`)

**Symptom (browser):** the thecrew Graphics page nav slot appears, `remoteEntry.js` loads (HTTP 200,
no `process`/React errors), but the host element shows
`Could not load thecrew: thecrew: remote does not export a \`mount\` function`.

**Environment:** the built shell (`make ui-preview`, :4173). Found live; invisible to the thecrew
unit suite because `mount.test.tsx` imports `mountPage` from `./remoteEntry` DIRECTLY — it never
exercises the shell's `pickMount` resolver, so the naming mismatch never surfaced in unit tests.

## Root cause

The shell's federation loader (`ui/src/features/ext-host/federation.ts` → `pickMount`) resolves the
PAGE mount by the name **`mount`** (named export, or `default.mount`) — the frozen federation contract,
byte-for-byte what proof-panel exports. thecrew's `remoteEntry.ts` exported the page as **`mountPage`**
(and `default = { mountPage, mountWidget }`). `pickMount` found no `mount`, so `loadRemoteMount` threw.

`mountWidget` was fine — the dashboard's `ExtWidget` renderer resolves the widget export by that exact
name, which thecrew did export.

## Fix

Export the page as `mount` (the shell's contract); keep `mountPage` as an alias for the unit suite:

```ts
export function mount(el, ctx, bridge) { injectStyles(); return mountPageImpl(el, ctx, bridge); }
export const mountPage = mount;             // back-compat: mount.test.tsx imports mountPage
export default { mount, mountWidget };       // default.mount is the fallback pickMount also accepts
```

`rust/extensions/thecrew/ui/src/remoteEntry.ts`.

## Prevention / follow-up

- The federation PAGE contract is `mount` (not `mountPage`) — the scaffold template
  (`crates/devkit/src/scaffold.rs`) and proof-panel both use `mount`; a new remote must too.
- A unit test that imports the export by name can't catch a shell-contract mismatch. The live-shell
  Playwright e2e is the guard (it goes through `pickMount`); consider a tiny unit assertion that the
  remoteEntry module has a `mount` export to fail fast without a browser.
