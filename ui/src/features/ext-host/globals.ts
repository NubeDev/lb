// The global keys the shell publishes its live React singletons under (ui-federation scope). The
// importmap shims (`ui/public/shims/*.mjs`) re-export from exactly these globals, and the shell's
// singleton publisher (`singletons.ts`) sets them at module eval — before any extension `remoteEntry.js`
// is dynamic-imported. An extension bundle externalises `react` / `react-dom` / `react-dom/client` /
// `react/jsx-runtime`; its bare imports resolve through the host importmap to the shims, which read
// these globals — so the extension binds to the host's SINGLE React instance. Two React copies break
// hooks/context (the "Invalid hook call" we hit with `@originjs/vite-plugin-federation`).
//
// Keep these strings in exact sync with `ui/public/shims/*.mjs`.

export const GLOBAL_REACT = "__lbReact";
export const GLOBAL_REACT_DOM = "__lbReactDom";
export const GLOBAL_REACT_DOM_CLIENT = "__lbReactDomClient";
export const GLOBAL_REACT_JSX_RUNTIME = "__lbReactJsxRuntime";
