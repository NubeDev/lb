// Load an extension's federated WIDGET remote at runtime and return its `mountWidget` export
// (widget-builder scope, follow-up 2 — a named export on the SAME remote entry as the page, one build).
// This is the widget analog of `ext-host/federation.ts`'s `loadRemoteMount`: a plain ESM dynamic import
// of the gateway-served `remoteEntry.js`, resolving React through the host import map (shared singleton).
// Used ONLY on the in-process tier (an allow-listed publisher key); untrusted widgets load inside the
// iframe sandbox instead.

// ── FROZEN WIDGET MOUNT CONTRACT — v4 (frames-in + theme) ────────────────────────────────────────
// The widget contract now lives in the standalone `@nube/ext-ui-sdk` (ext-out-of-tree scope, slice 2).
// The old "THREE mirrors that must move together" (this host type, each extension's `app/contract.ts`,
// the devkit template) collapse into ONE source: this file, the ext, and the template all import the
// SAME `@nube/ext-ui-sdk` types. Version-gated on `ctx.v` exactly as before (v2 bare teardown / v3
// `ctx.data` frames + `{update,teardown}` / v4 `ctx.theme`); the package definition is authoritative.
// Re-exported here (aliasing the package's `WidgetBridge` to the local name `WidgetBridgeContract`) so
// the in-shell importers of these names keep their import path unchanged.
export type {
  WidgetField,
  WidgetFrame,
  WidgetTheme,
  WidgetCtx,
  WidgetHandle,
  RemoteWidgetMount,
} from "@nube/ext-ui-sdk";
import type { RemoteWidgetMount, WidgetBridge } from "@nube/ext-ui-sdk";

/** The widget bridge — the leashed `call`/`watch` seam. Local alias of the package's `WidgetBridge`
 *  (a data tile needs neither; it renders `ctx.data`), kept so existing importers don't churn. */
export type WidgetBridgeContract = WidgetBridge;
export type { WidgetBridge } from "@nube/ext-ui-sdk";

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
