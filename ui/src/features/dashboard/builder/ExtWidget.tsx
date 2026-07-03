// ExtWidget — the `ext:<id>/<widget>` cell renderer (widget-builder scope; frames-in extended in
// ext-widget-source-binding scope). It mounts an extension-shipped widget tile, modelled on
// `proof-panel`'s federated page (ExtHost):
//
//   Load the extension's REAL remote and call its named `mountWidget(el, ctx, bridge, widgetId)`
//   export IN-PROCESS (a named export on the same remote entry, one build). It module-federates
//   against the shell's React singleton — native-feeling, and the only tier its bundle is built for
//   (the remote externalizes React expecting the shell import map). Installing the extension already
//   passed the publish/install capability gate, so the install IS the trust decision (see
//   trust.ts + debugging/frontend/ext-widget-iframe-tier-cannot-resolve-bare-react.md). The sandboxed
//   iframe tier is reserved for SCRIPTED author code (plot/d3/template), not installed widgets.
//
// TWO tile kinds share this one renderer, distinguished by the tile's manifest `data` flag:
//   - v2 self-fetching tile (`data` false): reaches data ONLY through the WidgetBridge (its
//     `[[widget]].scope ∩ grant`, re-checked at the host). It never receives resolved frames.
//   - v3 DATA tile (`data` true): a first-class VIEW over the cell's `sources[]`. The SHELL resolves
//     them through `viz.query` under the VIEWER's grant (per-target deny → empty frame, workspace
//     walled) and hands the tile `ctx.data` frames + `ctx.fieldConfig`. The tile RENDERS — it never
//     fetches, needs no read caps, never sees the token. On a data/vars/range tick the shell calls the
//     tile's `update(ctx)` handle in place (no re-mount). A data tile with no `update` handle falls
//     back to the v2 re-mount-on-configKey path.
//
// On unmount/uninstall the mount's cleanup runs and `watch` streams tear down (stateless eviction).
// An uninstalled ext renders "not installed".

import { useEffect, useMemo, useRef, useState } from "react";

import { widgetIdOf } from "@nube/source-picker";

import { gatewayUrl } from "@/lib/ipc/http";
import type { ExtRow } from "@/lib/ext/ext.api";
import type { Cell } from "@/lib/dashboard";
import { cellFieldConfig } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";
import { loadRemoteWidgetMount } from "./federationWidget";
import type { WidgetCtx, WidgetFrame, WidgetHandle } from "./federationWidget";
import { makeWidgetBridge, type WidgetBridge } from "./widgetBridge";
import { useVizFrames } from "./useVizFrames";

/** The current widget ctx contract version. v3 = frames-in (`ctx.data` + `ctx.fieldConfig`, optional
 *  `{ update, teardown }` return). A v2 tile that ignores the extra fields is unaffected; a v3 data
 *  tile reads them. The single version field the whole contract gates on. */
const WIDGET_CTX_V = 3;

/** An empty cell for `useVizFrames` when this tile is NOT a data tile — no primary target, so the hook
 *  makes no `viz.query` call (rules-of-hooks: the hook is always mounted, gated by `enabled` inside). */
const EMPTY_CELL: Cell = {
  i: "__ext_nodata__",
  x: 0,
  y: 0,
  w: 0,
  h: 0,
  widget_type: "chart",
  binding: {},
};

interface Props {
  /** `ext:<extension-id>/<widget-id>` — the cell view key. */
  viewKey: string;
  /** The installed extensions (from `ext.list`) — the source of the tile's entry/scope/data + trust input. */
  installed: ExtRow[];
  workspace: string;
  /** The shell-resolved variable scope (widget-config-vars Slice 3) — handed to the tile as
   *  `ctx.vars`/`ctx.timeRange`. The extension NEVER resolves identity or query vars itself. */
  scope?: VarScope;
  /** The cell's author-set `options` — forwarded to the tile as `ctx.options`. Inert config. */
  options?: Record<string, unknown>;
  /** The cell's `binding` (v1 series binding) — forwarded as `ctx.binding`. Inert config. */
  binding?: Record<string, unknown>;
  /** The full cell — a DATA tile reads its `sources[]`/`fieldConfig` to resolve `ctx.data`. Absent →
   *  treated as a non-data tile (v2 path). */
  cell?: Cell;
  /** Auto-refresh / live tick — re-resolves a data tile's frames and pushes them via `update(ctx)`. */
  refreshKey?: number;
}

/** Parse `ext:<id>/<widget>` into its parts. Returns null for a malformed key. */
function parseExtKey(viewKey: string): { ext: string; widget: string } | null {
  if (!viewKey.startsWith("ext:")) return null;
  const rest = viewKey.slice("ext:".length);
  const slash = rest.indexOf("/");
  if (slash < 0) return { ext: rest, widget: "" };
  return { ext: rest.slice(0, slash), widget: rest.slice(slash + 1) };
}

/** Render an installed extension-shipped widget tile, in-process (federated against the shell React). */
export function ExtWidget({
  viewKey,
  installed,
  workspace,
  scope = emptyScope(),
  options = {},
  binding = {},
  cell,
  refreshKey = 0,
}: Props) {
  const elRef = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);

  const parsed = parseExtKey(viewKey);
  const row = parsed ? installed.find((r) => r.ext === parsed.ext) : undefined;
  const tile = row?.widgets?.find((w) => w.entry && (parsed?.widget ? widgetIdOf(w) === parsed.widget : true));
  const bridge: WidgetBridge | null = tile ? makeWidgetBridge(tile.scope) : null;

  // Frames-in: a DATA tile (`tile.data`) resolves the cell's `sources[]` through `viz.query`, on the
  // SHARED read cache, under the VIEWER's grant. A non-data tile passes EMPTY_CELL → no call. The hook
  // is ALWAYS mounted (rules-of-hooks); `useVizFrames` no-ops without a resolvable target.
  const isData = !!tile?.data;
  const dataCell = isData && cell ? cell : EMPTY_CELL;
  const { frames } = useVizFrames(dataCell, scope, refreshKey);
  const fieldConfig = isData && cell ? cellFieldConfig(cell) : undefined;

  // The resolved time range from the built-ins (epoch ms strings → numbers; absent when no range).
  const fromMs = scope.builtins["__from"];
  const toMs = scope.builtins["__to"];
  const scopeKey = JSON.stringify(scope);
  // Serialize the author config so a cell-options edit re-mounts the tile with the new ctx. For a DATA
  // tile, `frames`/`fieldConfig` are NOT in this key — they flow through the in-place `update(ctx)` path
  // instead (no re-mount on every new sample), so a live tick never tears the tile down.
  const configKey = JSON.stringify({ options, binding });

  // Build the ctx the mount/update both receive. Memoized on the inputs so `update` fires only on a real
  // data/scope change. `data`/`fieldConfig` are present only for a data tile (a v2 tile ignores them).
  const ctx: WidgetCtx = useMemo(
    () => ({
      v: WIDGET_CTX_V,
      workspace,
      binding,
      options,
      vars: scope.values,
      builtins: scope.builtins,
      timeRange:
        fromMs !== undefined && toMs !== undefined ? { from: Number(fromMs), to: Number(toMs) } : undefined,
      data: isData ? (frames as WidgetFrame[]) : undefined,
      fieldConfig: isData ? fieldConfig : undefined,
    }),
    // eslint-disable-next-line react-hooks/exhaustive-deps -- serialized keys stand in for the object deps
    [workspace, configKey, scopeKey, isData, frames, fieldConfig, fromMs, toMs],
  );
  // A ref so the async mount closure always reads the LATEST ctx without re-running the mount effect.
  const ctxRef = useRef(ctx);
  ctxRef.current = ctx;

  // The tile's live handle (set once mounted): `{ update?, teardown? }` for a v3 tile, or a bare
  // teardown wrapped as `{ teardown }` for a v2 tile. The update effect below calls `update` in place.
  const handleRef = useRef<WidgetHandle | null>(null);

  useEffect(() => {
    if (!row || !tile || !bridge) return;
    const host = elRef.current;
    if (!host) return;

    // StrictMode double-invokes this effect (mount → cleanup → mount) in dev, and a var/config change
    // re-runs it too. The tile's `mount()` is ASYNC, so effect-run A can still be awaiting its remote
    // when run B starts. The old code mounted every run into the SAME `host` node and cleared it with
    // `host.replaceChildren()` — so run B wiped the DOM that run A's React root (`createRoot`) still
    // owned. When A's orphaned root later unmounted (on nav), it tried to remove nodes that were already
    // gone → "removeChild: not a child" INSIDE React's commit → the shell's commit aborts → nav wedges.
    // (The microtask defer never helped: it changed WHEN the orphan unmounted, not that it was orphaned.)
    //
    // Fix: give THIS effect-run its own child <div> under `host`. Run A and run B mount into DIFFERENT
    // nodes, so neither can wipe the other's DOM, and the async mount resolving after cleanup can't leak:
    // its teardown lives in the same `alive`/holder closure and always unmounts the root THIS run created.
    const slot = document.createElement("div");
    slot.className = "h-full w-full";
    host.appendChild(slot);

    let alive = true;
    const holder: { handle?: WidgetHandle } = {};
    setError(null);

    const remoteUrl = `${gatewayUrl()}/extensions/${encodeURIComponent(row.ext)}/ui/${tile.entry}`;
    (async () => {
      try {
        const mount = await loadRemoteWidgetMount(row.ext, remoteUrl);
        if (!alive) return;
        // Mount with the LATEST ctx (via the ref) — a v3 data tile gets its first frames here, then
        // `update(ctx)` for every subsequent tick without a re-mount.
        const ret = mount(slot, ctxRef.current, bridge, parsed?.widget ?? "");
        // Normalize the return: a v3 tile returns `{ update?, teardown? }`; a v2 tile returns a bare
        // teardown fn (or void). Either way we end with a `WidgetHandle` whose `teardown` disposes.
        const handle: WidgetHandle =
          typeof ret === "function" ? { teardown: ret } : ret && typeof ret === "object" ? ret : {};
        // If cleanup already ran while we were awaiting `mount`, tear this root down immediately —
        // otherwise stash it so cleanup and the update effect can reach it. Always disposed once.
        if (!alive) {
          handle.teardown?.();
        } else {
          holder.handle = handle;
          handleRef.current = handle;
        }
      } catch (e) {
        if (alive) setError(e instanceof Error ? e.message : String(e));
      }
    })();

    return () => {
      alive = false;
      // Unmount the tile's root (if it mounted) and remove OUR slot — never `host` itself, which the
      // shell owns. The tile's teardown only removes the children of OUR slot; because that slot is a
      // node no other effect-run and no shell commit touches, this is safe to run synchronously.
      try {
        holder.handle?.teardown?.();
      } catch {
        /* a tile that throws on its own teardown is already leaving; ignore */
      }
      if (handleRef.current === holder.handle) handleRef.current = null;
      slot.remove();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `scopeKey`/`configKey` re-mount the tile on a var/config change; `frames` do NOT (they flow through `update`)
  }, [row?.ext, tile?.entry, workspace, scopeKey, configKey]);

  // Live frames-in: push fresh ctx into the mounted tile in place. A v3 data tile that returned an
  // `update` handle re-renders WITHOUT a re-mount (the hard-won lifecycle above is untouched). A tile
  // that returned no `update` (a v2 tile, or a data tile that opted out) is a no-op here and instead
  // re-mounts via `configKey` when its author config changes — its frames are baked in at mount time.
  useEffect(() => {
    handleRef.current?.update?.(ctx);
  }, [ctx]);

  if (!parsed) return <Fallback>malformed widget key</Fallback>;
  if (!row || !row.enabled) return <Fallback>extension not installed</Fallback>;
  if (!tile || !bridge) return <Fallback>widget not found</Fallback>;

  return (
    <div className="h-full w-full" data-ext-widget={row.ext} data-tier="in-process">
      {error && <Fallback>could not load {row.ext}: {error}</Fallback>}
      <div ref={elRef} className="h-full w-full" />
    </div>
  );
}

// `widgetIdOf` (the tile-label → slug used to build/parse the `ext:<id>/<widget>` key) now lives in
// `@nube/source-picker` (imported above) — one slug function shared by the picker + this renderer, never
// two that can drift.

function Fallback({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex h-full w-full items-center justify-center p-3 text-center text-xs text-muted">
      {children}
    </div>
  );
}
