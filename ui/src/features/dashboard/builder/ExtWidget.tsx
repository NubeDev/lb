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

import { gatewayUrl } from "@/lib/ipc/http";
import type { ExtRow } from "@/lib/ext/ext.api";
import { loadRemoteWidgetMount } from "./federationWidget";
import { makeWidgetBridge, type WidgetBridge } from "./widgetBridge";

interface Props {
  /** `ext:<extension-id>/<widget-id>` — the cell view key. */
  viewKey: string;
  /** The installed extensions (from `ext.list`) — the source of the tile's entry/scope + trust input. */
  installed: ExtRow[];
  workspace: string;
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
export function ExtWidget({ viewKey, installed, workspace }: Props) {
  const elRef = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);

  const parsed = parseExtKey(viewKey);
  const row = parsed ? installed.find((r) => r.ext === parsed.ext) : undefined;
  const tile = row?.widgets?.find((w) => w.entry && (parsed?.widget ? widgetIdOf(w) === parsed.widget : true));
  const bridge: WidgetBridge | null = tile ? makeWidgetBridge(tile.scope) : null;

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
        unmount = mount(el, { workspace, binding: {}, options: {} }, bridge, parsed?.widget ?? "");
      } catch (e) {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      }
    })();

    return () => {
      cancelled = true;
      if (typeof unmount === "function") unmount();
      el.replaceChildren();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [row?.ext, tile?.entry, workspace]);

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

/** Derive a widget id from an `ExtUi` tile. The host narrows `[[widget]]` tiles but doesn't carry a
 *  separate id field today; we use the label slug as the stable widget id (matches the cell key the
 *  palette builds). Exported so the source picker builds the SAME `ext:<id>/<widget>` key the renderer
 *  parses — one slug function, never two that can drift. */
export function widgetIdOf(w: { label: string }): string {
  return w.label.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}

function Fallback({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex h-full w-full items-center justify-center p-3 text-center text-xs text-muted">
      {children}
    </div>
  );
}
