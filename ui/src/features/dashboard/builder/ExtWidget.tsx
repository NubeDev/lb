// ExtWidget — the `ext:<id>/<widget>` cell renderer (widget-builder scope, follow-up 1; the
// `widgets-scope.md` "render ext:<id> in a cell" open item, built here). It mounts an
// extension-shipped widget tile, modelled on `proof-panel`'s federated page (ExtHost), but routed BY
// TRUST TIER:
//
//   - An allow-listed publisher key → load the extension's REAL remote and call its named
//     `mountWidget(el, ctx, bridge, widgetId)` export IN-PROCESS (open Q2 lean: a named export on the
//     same remote entry, one build). Shares the shell React singleton, native-feeling.
//   - Everything else → render in a SANDBOXED IFRAME (a non-allow-listed key can NEVER federate
//     in-process even if its manifest asks).
//
// Either way the widget reaches data ONLY through the WidgetBridge (its `[[widget]].scope ∩ grant`,
// re-checked at the host). It never receives the token. On unmount/uninstall the mount's cleanup runs
// and `watch` streams tear down (stateless eviction). An uninstalled ext renders "not installed".

import { useEffect, useRef, useState } from "react";

import { gatewayUrl } from "@/lib/ipc/http";
import type { ExtRow } from "@/lib/ext/ext.api";
import { loadRemoteWidgetMount } from "./federationWidget";
import { extWidgetTier } from "./trust";
import { makeWidgetBridge, type WidgetBridge } from "./widgetBridge";
import { WidgetIframe } from "./WidgetIframe";

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

/** Render an extension-shipped widget tile, trust-tiered. */
export function ExtWidget({ viewKey, installed, workspace }: Props) {
  const elRef = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);

  const parsed = parseExtKey(viewKey);
  const row = parsed ? installed.find((r) => r.ext === parsed.ext) : undefined;
  const tile = row?.widgets?.find((w) => w.entry && (parsed?.widget ? widgetIdOf(w) === parsed.widget : true));
  // The publisher key gates the tier. `ext.list` rows don't carry the key today, so an extension is
  // trusted only if the shell's allow-list names it (by key id == ext id convention for the dev key);
  // absent that, iframe. This is the safe default — in-process is opt-in.
  const tier = row ? extWidgetTier(row.ext) : "iframe";
  const bridge: WidgetBridge | null = tile ? makeWidgetBridge(tile.scope) : null;

  useEffect(() => {
    if (!row || !tile || !bridge || tier !== "in-process") return;
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
  }, [row?.ext, tile?.entry, tier, workspace]);

  if (!parsed) return <Fallback>malformed widget key</Fallback>;
  if (!row || !row.enabled) return <Fallback>extension not installed</Fallback>;
  if (!tile || !bridge) return <Fallback>widget not found</Fallback>;

  if (tier === "iframe") {
    // Untrusted extension widget: the remote's bundle is loaded INTO the sandbox (the entry URL is the
    // engine the iframe template loads). We render it as a `template` whose code dynamic-imports the
    // remote and calls mountWidget — author code never touches the shell process.
    const code = remoteIframeCode(row.ext, tile.entry, parsed.widget);
    return (
      <div className="h-full w-full" data-ext-widget={row.ext} data-tier="iframe">
        <WidgetIframe engine="template" code={code} tools={tile.scope} bridge={bridge} />
      </div>
    );
  }

  return (
    <div className="h-full w-full" data-ext-widget={row.ext} data-tier="in-process">
      {error && <Fallback>could not load {row.ext}: {error}</Fallback>}
      <div ref={elRef} className="h-full w-full" />
    </div>
  );
}

/** Derive a widget id from an `ExtUi` tile. The host narrows `[[widget]]` tiles but doesn't carry a
 *  separate id field today; we use the label slug as the stable widget id (matches the cell key the
 *  palette builds). */
function widgetIdOf(w: { label: string }): string {
  return w.label.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}

/** The iframe `template` code that dynamic-imports an untrusted remote inside the sandbox and mounts
 *  its widget. The remote runs in the opaque origin; it reaches data only via the posted bridge. */
function remoteIframeCode(ext: string, entry: string, widget: string): string {
  const url = `${gatewayUrl()}/extensions/${encodeURIComponent(ext)}/ui/${entry}`;
  return `const m = await import(${JSON.stringify(url)});\n` +
    `const mount = m.mountWidget || m.mount;\n` +
    `if (typeof mount === "function") mount(el, { workspace: "", binding: {}, options: {} }, bridge, ${JSON.stringify(widget)});\n` +
    `else el.textContent = "remote has no mountWidget";`;
}

function Fallback({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex h-full w-full items-center justify-center p-3 text-center text-xs text-muted">
      {children}
    </div>
  );
}
