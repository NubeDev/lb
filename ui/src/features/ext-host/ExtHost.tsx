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
    const host = elRef.current;
    if (!host) return;

    // The extension's `mount()` is ASYNC and it owns its OWN React root (`createRoot`). StrictMode
    // double-invokes this effect (mount → cleanup → mount) in dev, so run A can still be awaiting its
    // remote when run B starts. Mounting every run into the SAME `host` and clearing it with
    // `host.replaceChildren()` (the old code) let run B wipe the DOM run A's root still owned — and when
    // A's orphaned root later unmounted (on nav), it removed nodes already gone → "removeChild: not a
    // child" INSIDE React's commit → the shell's commit aborts → the sidebar/nav wedge. (Deferring the
    // unmount to a microtask never helped: it changed WHEN the orphan unmounted, not that it was orphaned.)
    //
    // Fix: give THIS effect-run its own child <div> under `host`. Runs mount into DIFFERENT nodes, so
    // neither wipes the other's DOM, and a mount that resolves after cleanup can't leak — its teardown
    // lives in the same `alive`/holder closure and always unmounts the root THIS run created.
    const slot = document.createElement("div");
    slot.className = "h-full w-full";
    host.appendChild(slot);

    let alive = true;
    const holder: { unmount?: () => void } = {};
    setError(null);

    // The manifest `entry` (e.g. `assets/remoteEntry.js`) is the federation container, served under
    // the gateway's per-extension UI route.
    const remoteUrl = `${gatewayUrl()}/extensions/${encodeURIComponent(ext)}/ui/${ui.entry}`;
    (async () => {
      try {
        const mount = await loadRemoteMount(ext, remoteUrl);
        if (!alive) return;
        const teardown = mount(slot, { workspace }, makeBridge(ui.scope));
        if (!alive) {
          if (typeof teardown === "function") teardown();
        } else {
          holder.unmount = typeof teardown === "function" ? teardown : undefined;
        }
      } catch (e) {
        if (alive) setError(e instanceof Error ? e.message : String(e));
      }
    })();

    return () => {
      alive = false;
      // Unmount the extension's root (if it mounted) and remove OUR slot — never `host`, which the shell
      // owns. The root's `unmount()` only removes children of OUR slot, a node no other run and no shell
      // commit touches, so this is safe to run synchronously (no shared-DOM double-remove); no defer.
      try {
        holder.unmount?.();
      } catch {
        /* extension unmount threw — already leaving the page; ignore */
      }
      slot.remove();
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
