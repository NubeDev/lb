// The page host (ui-federation scope) — renders an installed extension's FULL PAGE in a shell surface.
// Trusted tier (this slice): load the extension's REAL Module Federation remote (`remoteEntry.js`)
// served by the gateway and call its `mount(el, ctx, bridge)` in-process. Because the remote shares
// the shell's `react`/`react-dom` singletons (the federation HOST config in `vite.config.ts`), the
// page renders against the SAME React — native-feeling, one runtime, no bundled copy. The page reaches
// data ONLY through `bridge` (host-mediated, cap-checked) — it never gets the token.
//
// The untrusted iframe-sandbox tier is the immediate follow-up (same `mount` contract, postMessage
// transport); a non-allow-listed publisher would render there instead of in-process.

import { useEffect, useRef, useState } from "react";

import { gatewayUrl } from "@/lib/ipc/http";
import type { ExtUi } from "@/lib/ext/ext.api";
import { makeBridge } from "./bridge";
import { loadRemoteMount } from "./federation";

interface Props {
  ext: string;
  ui: ExtUi;
  workspace: string;
}

/** Load + mount extension `ext`'s federated page. Shows a load/error state honestly (no fake content). */
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

    // The manifest `entry` (e.g. `assets/remoteEntry.js`) is the federation container, served under
    // the gateway's per-extension UI route.
    const remoteUrl = `${gatewayUrl()}/extensions/${encodeURIComponent(ext)}/ui/${ui.entry}`;
    (async () => {
      try {
        const mount = await loadRemoteMount(ext, remoteUrl);
        if (cancelled) return;
        unmount = mount(el, { workspace }, makeBridge(ui.scope));
      } catch (e) {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      }
    })();

    return () => {
      cancelled = true;
      // Let the extension tear down its own tree first (it may own a React root in `el`). Guard both
      // steps: a throw here runs during the shell's own render/commit and would surface as React's
      // "synchronously unmount a root while rendering" / "removeChild: not a child" errors, unwinding
      // the shell. Swallow — teardown of a page we're navigating away from must never crash the shell.
      try {
        if (typeof unmount === "function") unmount();
      } catch {
        /* extension unmount threw — already leaving the page; ignore */
      }
      try {
        el.replaceChildren();
      } catch {
        /* DOM already reconciled away by the extension's own root; ignore */
      }
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
