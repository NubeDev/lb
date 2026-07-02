// ExtWidget — the `ext:<id>/<widget>` cell renderer (widget-builder scope, follow-up 1; the
// `widgets-scope.md` "render ext:<id> in a cell" open item, built here). It mounts an
// extension-shipped widget tile, modelled on `proof-panel`'s federated page (ExtHost):
//
//   Load the extension's REAL remote and call its named `mountWidget(el, ctx, bridge, widgetId)`
//   export IN-PROCESS (a named export on the same remote entry, one build). It module-federates
//   against the shell's React singleton — native-feeling, and the only tier its bundle is built for
//   (the remote externalizes React expecting the shell import map). Installing the extension already
//   passed the publish/install capability gate, so the install IS the trust decision (see
//   trust.ts + debugging/frontend/ext-widget-iframe-tier-cannot-resolve-bare-react.md). The sandboxed
//   iframe tier is reserved for SCRIPTED author code (plot/d3/template), not installed widgets.
//
// The widget reaches data ONLY through the WidgetBridge (its `[[widget]].scope ∩ grant`, re-checked
// at the host). It never receives the token. On unmount/uninstall the mount's cleanup runs and
// `watch` streams tear down (stateless eviction). An uninstalled ext renders "not installed".

import { useEffect, useRef, useState } from "react";

import { widgetIdOf } from "@nube/source-picker";

import { gatewayUrl } from "@/lib/ipc/http";
import type { ExtRow } from "@/lib/ext/ext.api";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";
import { loadRemoteWidgetMount } from "./federationWidget";
import { makeWidgetBridge, type WidgetBridge } from "./widgetBridge";

/** The additive v2 widget ctx version (widget-config-vars Slice 3). A v1 widget that ignores `vars`/
 *  `timeRange` is unaffected; a v2 widget reads them. Frozen alongside the shared vars library. */
const WIDGET_CTX_V = 2;

interface Props {
  /** `ext:<extension-id>/<widget-id>` — the cell view key. */
  viewKey: string;
  /** The installed extensions (from `ext.list`) — the source of the tile's entry/scope + trust input. */
  installed: ExtRow[];
  workspace: string;
  /** The shell-resolved variable scope (widget-config-vars Slice 3) — handed to the tile as
   *  `ctx.vars`/`ctx.timeRange`. The extension NEVER resolves identity or query vars itself. */
  scope?: VarScope;
  /** The cell's author-set `options` (e.g. `{sceneId}`) — forwarded to the tile as `ctx.options` so the
   *  scope's intended `{view, options:{sceneId}}` cell shape reaches the widget. The tile still reads
   *  data only through the bridge; `options` is inert configuration, not a capability. */
  options?: Record<string, unknown>;
  /** The cell's `binding` (v1 series binding) — forwarded as `ctx.binding` for a tile that binds a
   *  single series through its cell. Inert config, re-checked at the host on any bridged read. */
  binding?: Record<string, unknown>;
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
}: Props) {
  const elRef = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);

  const parsed = parseExtKey(viewKey);
  const row = parsed ? installed.find((r) => r.ext === parsed.ext) : undefined;
  const tile = row?.widgets?.find((w) => w.entry && (parsed?.widget ? widgetIdOf(w) === parsed.widget : true));
  const bridge: WidgetBridge | null = tile ? makeWidgetBridge(tile.scope) : null;
  // The resolved time range from the built-ins (epoch ms strings → numbers; absent when no range).
  const fromMs = scope.builtins["__from"];
  const toMs = scope.builtins["__to"];
  const scopeKey = JSON.stringify(scope);
  // Serialize the author config so a cell-options edit (e.g. changing `sceneId`) re-mounts the tile with
  // the new ctx — same pattern as `scopeKey`. `options`/`binding` are inert config; the host re-gates any
  // bridged read regardless of what the tile does with them.
  const configKey = JSON.stringify({ options, binding });

  useEffect(() => {
    if (!row || !tile || !bridge) return;
    let unmount: void | (() => void);
    let cancelled = false;
    const el = elRef.current;
    if (!el) return;
    el.replaceChildren();
    setError(null);

    const remoteUrl = `${gatewayUrl()}/extensions/${encodeURIComponent(row.ext)}/ui/${tile.entry}`;
    (async () => {
      try {
        const mount = await loadRemoteWidgetMount(row.ext, remoteUrl);
        if (cancelled) return;
        // v2 ctx: additive `vars` (resolved selections) + `timeRange` (the URL range, shell-resolved
        // from the token — the tile never resolves identity/query vars itself). `v` marks the contract.
        const ctx = {
          v: WIDGET_CTX_V,
          workspace,
          binding,
          options,
          vars: scope.values,
          builtins: scope.builtins,
          timeRange:
            fromMs !== undefined && toMs !== undefined
              ? { from: Number(fromMs), to: Number(toMs) }
              : undefined,
        };
        unmount = mount(el, ctx, bridge, parsed?.widget ?? "");
      } catch (e) {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      }
    })();

    return () => {
      cancelled = true;
      if (typeof unmount === "function") unmount();
      el.replaceChildren();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `scopeKey`/`configKey` re-mount the tile on a var/range/config change
  }, [row?.ext, tile?.entry, workspace, scopeKey, configKey]);

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
