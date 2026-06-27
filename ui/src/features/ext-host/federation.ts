// Load a federated extension remote at RUNTIME and return its `mount` export (ui-federation scope).
// This is the REAL Module Federation seam: the extension ships a `remoteEntry.js` container that
// declares `react`/`react-dom` as shared singletons; the shell (the federation HOST, see
// `vite.config.ts`) provides those singletons, so the remote's `mount` renders in-process against
// the shell's SAME React — no bundled second copy, no hook-dispatcher mismatch. The remote looks
// native because it literally shares the host's runtime.
//
// Remotes are not known at build time (an extension is installed later, served by the gateway), so
// we register each one DYNAMICALLY via the federation runtime that `@originjs/vite-plugin-federation`
// injects into the host bundle (`__federation_method_*`). One remote name per extension id.

// The runtime helpers the federation plugin emits into the host. Declared here (not imported) because
// the plugin injects a virtual module resolved only in a real Vite build; this keeps the types local.
type FederationShared = Record<string, unknown>;
declare global {
  interface Window {
    __FEDERATION_SHELL_REGISTERED__?: Set<string>;
  }
}

interface FederationRuntime {
  __federation_method_setRemote: (
    name: string,
    remote: { url: () => Promise<string> | string; format: "esm"; from: "vite" },
  ) => void;
  __federation_method_getRemote: (name: string, expose: string) => Promise<FederationShared>;
}

/** The mount contract every extension remote must expose as `./mount` (ui-federation scope). */
export type RemoteMount = (
  el: HTMLElement,
  ctx: { workspace: string },
  bridge: { call: <T = unknown>(tool: string, args?: Record<string, unknown>) => Promise<T> },
) => void | (() => void);

/** Import the federation runtime the host bundle carries. Isolated behind a function so a jsdom/test
 *  environment (where the virtual module is absent) fails loudly rather than at module-eval time. */
async function runtime(): Promise<FederationRuntime> {
  // @ts-expect-error — virtual module injected by @originjs/vite-plugin-federation at build time.
  return (await import("__federation__")) as FederationRuntime;
}

/**
 * Register `ext`'s remote (served at `remoteEntryUrl`) and return its `./mount`. Idempotent per
 * extension id — re-opening the same page reuses the already-registered container (the federation
 * runtime caches it), so React stays a single shared instance across mounts.
 */
export async function loadRemoteMount(ext: string, remoteEntryUrl: string): Promise<RemoteMount> {
  const fed = await runtime();
  const registered = (window.__FEDERATION_SHELL_REGISTERED__ ??= new Set<string>());
  if (!registered.has(ext)) {
    fed.__federation_method_setRemote(ext, {
      url: () => remoteEntryUrl,
      format: "esm",
      from: "vite",
    });
    registered.add(ext);
  }
  const mod = (await fed.__federation_method_getRemote(ext, "./mount")) as { mount?: RemoteMount };
  if (typeof mod.mount !== "function") {
    throw new Error(`${ext}: federated remote does not expose ./mount`);
  }
  return mod.mount;
}
