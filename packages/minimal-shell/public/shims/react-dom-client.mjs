const R = (globalThis).__lbReactDomClient;
if (!R) throw new Error("minimal-shell: globalThis.__lbReactDomClient not set");
export default R.default ?? R;
export const { createRoot, hydrateRoot, version } = R;
