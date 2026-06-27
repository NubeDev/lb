// Importmap shim — see `react.mjs` for the contract. Re-exports the host's `react-dom` so extensions
// externalising it bind to the host instance.
const RD = /** @type {any} */ (globalThis).__lbReactDom;
if (!RD) {
  throw new Error("lb react-dom-shim: globalThis.__lbReactDom is unset.");
}
export default RD.default ?? RD;
export const { createPortal, flushSync, version } = RD;
