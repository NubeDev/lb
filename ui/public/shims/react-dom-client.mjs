// Importmap shim for `react-dom/client` — for extensions that call `createRoot` (the `mount` contract
// renders with it). Re-exports from the host's react-dom/client so the root shares the host's React.
const RDC = /** @type {any} */ (globalThis).__lbReactDomClient;
if (!RDC) {
  throw new Error("lb react-dom-client-shim: globalThis.__lbReactDomClient is unset.");
}
export const { createRoot, hydrateRoot } = RDC;
