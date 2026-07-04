# react-native-svg breaks the RN-Web preview (native-only Fabric imports)

**Area:** app / web preview
**Status:** resolved (preview shim)

## Symptom

Adding react-native-svg (for `GaugeRing`) broke the Vite RN-Web preview (`vite.config.web.mts`).
esbuild dep-optimize failed, cascading through several native-only modules as each was patched:

```
ERROR: Could not read from file: …/react-native-web/Libraries/Utilities/codegenNativeComponent
ERROR: No matching export in react-native-web/dist/index.js for import "TurboModuleRegistry"
```

The page loaded a light-grey blank (RN root never mounted).

## Root cause

react-native-svg's Fabric layer (`lib/module/fabric/*NativeComponent.js`,
`NativeSvgRenderableModule.js`) imports native-only RN internals — `codegenNativeComponent`,
`TurboModuleRegistry` — that **react-native-web does not expose**. The RN-Web Vite plugin's
`enforce: 'pre'` esbuild alias also rewrites `react-native/…/codegenNativeComponent` to a
non-absolute path the pre-bundler can't read. Patching imports one at a time just surfaced the next
missing native module — a losing chase.

## Fix

Alias `react-native-svg` → a small DOM-`<svg>` shim for the **preview only**
(`web/svg-shim/index.tsx`), in `vite.config.web.mts`:

```ts
{ find: 'react-native-svg', replacement: here('web/svg-shim') },
```

The shim exports the same element names our components use (`Svg`, `Circle`, `Rect`, `Path`, `G`,
`Text`, gradients, …) as thin wrappers over the browser's own `<svg>` elements. On device the real
react-native-svg is used (this alias is web-only). Per rule 9 this is a **preview rendering shim, not
a fake backend** — it reimplements no node behavior, only maps RN svg element names to DOM svg.

## Verification / regression guard

Puppeteer screenshot of a kit gallery: `GaugeRing` renders **60 `<circle>` elements** with the mint
lead-arc (`document.querySelectorAll('circle').length === 60`) and no console pageerror. That DOM
assertion is the regression check — if the shim loses an element the components use, the count drops
or the render errors.
