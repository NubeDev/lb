const R = (globalThis).__lbReactDom;
if (!R) throw new Error("minimal-shell: globalThis.__lbReactDom not set");
export default R.default ?? R;
export const { createPortal, flushSync, version } = R;
