/// <reference types="vite/client" />

// `?inline` CSS imports resolve to the compiled stylesheet as a string (injected at runtime by
// remoteEntry.ts). Declared so the lib build + tsc see a `string` default export.
declare module "*.css?inline" {
  const css: string;
  export default css;
}

// The vendored editor's already-compiled stylesheet is imported `?raw` (verbatim bytes, no PostCSS).
declare module "*.css?raw" {
  const css: string;
  export default css;
}
