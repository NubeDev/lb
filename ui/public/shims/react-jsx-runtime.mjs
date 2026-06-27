// Importmap shim — extension bundles compiled with the modern JSX transform import `jsx`/`jsxs`/
// `Fragment` from `react/jsx-runtime`. Re-export from the host's published runtime so they use the
// host's React internals (one dispatcher, no "Invalid hook call").
const J = /** @type {any} */ (globalThis).__lbReactJsxRuntime;
if (!J) {
  throw new Error("lb jsx-runtime-shim: globalThis.__lbReactJsxRuntime is unset.");
}
export const { jsx, jsxs, jsxDEV, Fragment } = J;
