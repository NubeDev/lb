// Vite's `?raw` import suffix returns a module's contents as a string. The
// package's tsconfig doesn't pull in `vite/client`, so declare it locally.
// Used by ./ui/tsLibs.ts to embed the TypeScript standard-library .d.ts files.
declare module "*?raw" {
  const content: string;
  export default content;
}
