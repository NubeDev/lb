// Load an extension's federated WIDGET remote at runtime and return its `mountWidget` export
// (widget-builder scope, follow-up 2 — a named export on the SAME remote entry as the page, one build).
// This is the widget analog of `ext-host/federation.ts`'s `loadRemoteMount`: a plain ESM dynamic import
// of the gateway-served `remoteEntry.js`, resolving React through the host import map (shared singleton).
// Used ONLY on the in-process tier (an allow-listed publisher key); untrusted widgets load inside the
// iframe sandbox instead.

// ── FROZEN WIDGET MOUNT CONTRACT — v3 (frames-in) ────────────────────────────────────────────────
// This type is one of THREE mirrors that MUST move together (ext-widget-source-binding scope):
//   1. this host-side `RemoteWidgetMount` / `WidgetCtx`,
//   2. the extension-side `app/contract.ts` copies (proof-panel, echarts-panel, thecrew, …),
//   3. the ext-sdk / devkit template.
// It is strictly ADDITIVE over v2: v3 adds `ctx.data` (resolved frames) + `ctx.fieldConfig`, and
// widens the return to an optional `{ update?, teardown? }` object. Version-gate on `ctx.v`.
//   - v2 tile: reads `binding`/`options`/`vars`/`timeRange`, returns a bare teardown fn (or void).
//   - v3 data tile: reads `ctx.data` frames (the shell resolved its `sources[]` via `viz.query`),
//     returns `{ update, teardown }` so live/vars/range ticks re-render in place (no re-mount).
// A v2 tile under this v3 host is byte-identical: extra ctx fields are ignored, a fn return still
// tears down. See ExtWidget.tsx for how the shell chooses update-vs-remount.

/** One resolved field in a frame — the `lb-viz` `Frame.fields[]` shape (mirrors `useVizQuery.ts`). */
export interface WidgetField {
  name: string;
  type?: string;
  values: unknown[];
}

/** A resolved data frame handed to a v3 data tile via `ctx.data`. The `lb-viz` frame shape the
 *  built-in renderers consume — a public contract the moment it reaches a third party (freeze it,
 *  version it with `ctx.v`). */
export interface WidgetFrame {
  refId?: string;
  name?: string;
  fields: WidgetField[];
  length?: number;
}

/** The v3 widget mount ctx. v2 fields (`binding`/`options`/`vars`/`builtins`/`timeRange`) remain;
 *  v3 adds `data` (resolved frames — present iff the tile's manifest set `data = true`) and
 *  `fieldConfig` (the Field-tab options, passed through so the tile drives its render from them). */
export interface WidgetCtx {
  /** Contract version. `3` = frames-in; a tile gates its use of `data`/`fieldConfig` on `v >= 3`. */
  v: number;
  workspace: string;
  binding: Record<string, unknown>;
  options: Record<string, unknown>;
  vars?: Record<string, unknown>;
  builtins?: Record<string, unknown>;
  timeRange?: { from: number; to: number };
  /** v3 (data tiles only): the shell-resolved frames for the cell's `sources[]`. Absent for a v2
   *  tile or a data tile with no bound sources. The tile RENDERS these — it never fetches. */
  data?: WidgetFrame[];
  /** v3 (data tiles only): the cell's Field-tab `fieldConfig` (units/decimals/thresholds/legend/…). */
  fieldConfig?: unknown;
}

/** The widget bridge — the leashed `call`/`watch` seam (a data tile needs neither; it renders `ctx.data`). */
export interface WidgetBridgeContract {
  call: <T = unknown>(tool: string, args?: Record<string, unknown>) => Promise<T>;
  watch: (tool: string, args: Record<string, unknown>, onEvent: (e: unknown) => void) => () => void;
}

/** A v3 tile MAY return this object instead of a bare teardown: `update(ctx)` re-renders in place on
 *  a data/vars/range tick (no re-mount), `teardown()` disposes on unmount. A v2 tile returns a bare
 *  function (or void) and the shell falls back to re-mount-on-configKey. */
export interface WidgetHandle {
  update?: (ctx: WidgetCtx) => void;
  teardown?: () => void;
}

/** The widget mount contract — like the page `mount`, plus the `widgetId` selecting which `[[widget]]`
 *  tile to render. The bridge may `call` AND `watch`. Returns void, a bare teardown (v2), or a
 *  `{ update, teardown }` handle (v3). */
export type RemoteWidgetMount = (
  el: HTMLElement,
  ctx: WidgetCtx,
  bridge: WidgetBridgeContract,
  widgetId: string,
) => void | (() => void) | WidgetHandle;

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
