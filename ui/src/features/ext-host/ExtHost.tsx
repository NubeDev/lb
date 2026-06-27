// The page host (ui-federation scope) — renders an installed extension's FULL PAGE in a shell surface.
// Trusted tier (this slice): dynamic-`import()` the extension's ESM bundle from the gateway and call
// its `mount(el, ctx, bridge)` in-process, so it shares the shell's React/DOM and feels native. The
// page reaches data ONLY through `bridge` (host-mediated, cap-checked) — it never gets the token.
//
// The untrusted iframe-sandbox tier is the immediate follow-up (same `mount` contract, postMessage
// transport); a non-allow-listed publisher would render there instead of in-process.

import { useEffect, useRef, useState } from "react";

import { gatewayUrl } from "@/lib/ipc/http";
import type { ExtUi } from "@/lib/ext/ext.api";
import { makeBridge } from "./bridge";

/** The mount contract an extension UI bundle must export. */
type MountFn = (
  el: HTMLElement,
  ctx: { workspace: string },
  bridge: ReturnType<typeof makeBridge>,
) => void | (() => void);

interface Props {
  ext: string;
  ui: ExtUi;
  workspace: string;
}

/** Load + mount extension `ext`'s page. Shows a load/error state honestly (no fake content). */
export function ExtHost({ ext, ui, workspace }: Props) {
  const elRef = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let unmount: void | (() => void);
    let cancelled = false;
    const el = elRef.current;
    if (!el) return;
    el.replaceChildren();
    setError(null);

    const url = `${gatewayUrl()}/extensions/${encodeURIComponent(ext)}/ui/${ui.entry}`;
    (async () => {
      try {
        const mod: { mount?: MountFn } = await import(/* @vite-ignore */ url);
        if (cancelled) return;
        if (typeof mod.mount !== "function") {
          throw new Error("bundle does not export mount(el, ctx, bridge)");
        }
        unmount = mod.mount(el, { workspace }, makeBridge(ui.scope));
      } catch (e) {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      }
    })();

    return () => {
      cancelled = true;
      if (typeof unmount === "function") unmount();
      el.replaceChildren();
    };
  }, [ext, ui.entry, ui.scope, workspace]);

  return (
    <div className="h-full w-full">
      {error && (
        <div className="m-4 rounded-md border border-border bg-panel p-4 text-sm text-muted">
          Could not load <span className="text-accent">{ext}</span>: {error}
        </div>
      )}
      <div ref={elRef} className="h-full w-full" data-ext-host={ext} />
    </div>
  );
}
