// Lazy-load a self-hosted `@fontsource` family on selection — the ONE place font woff2 is fetched, so
// fonts never ship in the main bundle and never load unless a member picks one (the bundle-check in the
// tests asserts this). Each family is a dynamic `import()` of its latin 400+600 CSS, which Vite splits
// into its own chunk; the CSS `@font-face` self-hosts the woff2 (no CDN — the shell boots offline / in
// Tauri). A system stack has no loader entry (nothing to fetch). Idempotent + fire-and-forget: a failed
// import just leaves the CSS `font-family` fallback in place, never throws into the render path.
//
// One responsibility: font id → lazy import of its bundled faces.

/** id → the dynamic imports for that family's latin 400/600 faces. Static so Vite can code-split each;
 *  a family absent here is a system stack (no download). Weights kept to regular + semibold (UI needs). */
const LOADERS: Record<string, ReadonlyArray<() => Promise<unknown>>> = {
  inter: [() => import("@fontsource/inter/latin-400.css"), () => import("@fontsource/inter/latin-600.css")],
  geist: [() => import("@fontsource/geist-sans/latin-400.css"), () => import("@fontsource/geist-sans/latin-600.css")],
  "ibm-plex-sans": [
    () => import("@fontsource/ibm-plex-sans/latin-400.css"),
    () => import("@fontsource/ibm-plex-sans/latin-600.css"),
  ],
  "source-serif-4": [
    () => import("@fontsource/source-serif-4/latin-400.css"),
    () => import("@fontsource/source-serif-4/latin-600.css"),
  ],
  "jetbrains-mono": [
    () => import("@fontsource/jetbrains-mono/latin-400.css"),
    () => import("@fontsource/jetbrains-mono/latin-600.css"),
  ],
  "ibm-plex-mono": [
    () => import("@fontsource/ibm-plex-mono/latin-400.css"),
    () => import("@fontsource/ibm-plex-mono/latin-600.css"),
  ],
};

const loaded = new Set<string>();

/** Load a family's faces if self-hosted and not already loaded. Returns immediately for a system stack
 *  or an already-loaded family. Never rejects into the caller — a failed fetch just keeps the fallback. */
export function loadFont(id: string): void {
  const loaders = LOADERS[id];
  if (!loaders || loaded.has(id)) return;
  loaded.add(id);
  for (const load of loaders) {
    void load().catch(() => {
      // A blocked/offline fetch leaves the CSS `font-family` fallback stack — allow a retry next time.
      loaded.delete(id);
    });
  }
}
