// Load an extension UI remote at RUNTIME and return its `mount` export (ui-federation scope).
//
// The seam is a plain ESM dynamic import — NOT `@originjs/vite-plugin-federation`. The gateway serves
// the extension's `remoteEntry.js` as a static ESM module; we `import(url)` it directly. The remote
// externalises `react`/`react-dom`/`react-dom/client`/`react/jsx-runtime`, so its bare imports resolve
// through the host import map (index.html) to the shims, which re-export the shell's SINGLE React
// (published on `globalThis.__lb*` by `singletons.ts`). The remote therefore renders in-process against
// the host's SAME React — no bundled second copy, no hook-dispatcher mismatch. This is the rubix-cube
// import-map pattern; it replaces the @originjs plugin, whose dynamic-remote share scope shipped a
// second React and broke hooks ("Invalid hook call"). See
// debugging/extensions/federated-remote-fails-in-dev-server.md.

// The page mount contract now lives in the standalone `@nube/ext-ui-sdk` — the single authoritative
// source the old per-extension `app/contract.ts` copies collapse into (ext-out-of-tree scope, slice 2).
// Re-exported so the in-shell importers of `RemoteMount` keep their import path unchanged.
export type { RemoteMount } from "@nube/ext-ui-sdk";
import type { RemoteMount } from "@nube/ext-ui-sdk";

/** A loaded remote module. The remote exports `mount` (named); some bundlers also surface it as the
 *  default export, so we accept either shape. */
interface RemoteModule {
  mount?: RemoteMount;
  default?: RemoteMount | { mount?: RemoteMount };
}

/** Resolve the `mount` function from a loaded remote module, tolerating named or default placement. */
function pickMount(mod: RemoteModule): RemoteMount | undefined {
  if (typeof mod.mount === "function") return mod.mount;
  const d = mod.default;
  if (typeof d === "function") return d;
  if (d && typeof d === "object" && typeof d.mount === "function") return d.mount;
  return undefined;
}

/**
 * Dynamic-import `ext`'s remote (served at `remoteEntryUrl`) and return its `mount`. The browser caches
 * the module by URL, so re-opening the same page reuses the already-evaluated remote — React stays a
 * single shared instance across mounts. `@vite-ignore` keeps Vite from trying to bundle a runtime URL.
 */
export async function loadRemoteMount(ext: string, remoteEntryUrl: string): Promise<RemoteMount> {
  const mod = (await import(/* @vite-ignore */ remoteEntryUrl)) as RemoteModule;
  const mount = pickMount(mod);
  if (typeof mount !== "function") {
    throw new Error(`${ext}: remote does not export a \`mount\` function`);
  }
  return mount;
}
