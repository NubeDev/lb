// Load an extension's federated WIDGET remote at runtime and return its `mountWidget` export
// (widget-builder scope, follow-up 2 — a named export on the SAME remote entry as the page, one build).
// This is the widget analog of `ext-host/federation.ts`'s `loadRemoteMount`: a plain ESM dynamic import
// of the gateway-served `remoteEntry.js`, resolving React through the host import map (shared singleton).
// Used ONLY on the in-process tier (an allow-listed publisher key); untrusted widgets load inside the
// iframe sandbox instead.

/** The widget mount contract — like the page `mount`, plus the `widgetId` selecting which `[[widget]]`
 *  tile to render, and a v2 `ctx`/`bridge` (the bridge may `call` AND `watch`, reads or writes). */
export type RemoteWidgetMount = (
  el: HTMLElement,
  ctx: { workspace: string; binding: Record<string, unknown>; options: Record<string, unknown> },
  bridge: {
    call: <T = unknown>(tool: string, args?: Record<string, unknown>) => Promise<T>;
    watch: (tool: string, args: Record<string, unknown>, onEvent: (e: unknown) => void) => () => void;
  },
  widgetId: string,
) => void | (() => void);

interface RemoteModule {
  mountWidget?: RemoteWidgetMount;
  default?: { mountWidget?: RemoteWidgetMount };
}

function pickWidgetMount(mod: RemoteModule): RemoteWidgetMount | undefined {
  if (typeof mod.mountWidget === "function") return mod.mountWidget;
  const d = mod.default;
  if (d && typeof d === "object" && typeof d.mountWidget === "function") return d.mountWidget;
  return undefined;
}

/** Dynamic-import `ext`'s remote and return its `mountWidget`. Throws if the remote exposes none. */
export async function loadRemoteWidgetMount(
  ext: string,
  remoteEntryUrl: string,
): Promise<RemoteWidgetMount> {
  const mod = (await import(/* @vite-ignore */ remoteEntryUrl)) as RemoteModule;
  const mount = pickWidgetMount(mod);
  if (typeof mount !== "function") {
    throw new Error(`${ext}: remote does not export a \`mountWidget\` function`);
  }
  return mount;
}
