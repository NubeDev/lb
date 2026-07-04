// Web replacement for src/polyfills.ts. The browser's native `fetch` already streams response
// bodies (res.body is a real ReadableStream) and ships TextDecoder — so the SSE client works with
// zero polyfilling here. This module exists only so the `react-native/…/PolyfillFunctions` import
// in index.web.tsx resolves to a harmless no-op (via the vite alias).

export function polyfillGlobal(): void {
  /* no-op on web */
}
