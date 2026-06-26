// The native-tier view — install, supervise, restart, and stop an OS-process (Tier-2) sidecar.
// Layout + wiring only; data lives in useNative (FILE-LAYOUT). This is the UI face of the S7 exit
// gate's second half: a native sidecar is supervised and restarts cleanly — the restart COUNT and
// the live running flag are surfaced so the user sees the supervision working. A user without the
// grant is denied — the same `mcp:native.<verb>:call` gate the Rust native tests prove on the
// backend, surfaced to the user.

import { Cpu } from "lucide-react";

import { useNative } from "./useNative";

interface Props {
  ws: string;
  extId: string;
  /** The current user's principal (demo session identity until real login lands). */
  author: string;
  /** The caller's held capabilities (the grant the node checks; demo until real tokens). */
  caps: string[];
}

export function NativeView({ ws, extId, author, caps }: Props) {
  const { status, error, install, restart, stop } = useNative(ws, extId, author, caps);

  return (
    <section className="flex h-full flex-col bg-bg">
      <header className="flex items-center gap-2 border-b border-border px-4 py-3">
        <Cpu size={16} className="text-muted" />
        <h1 className="text-sm font-medium">Native sidecar — {extId}</h1>
        <span className="ml-auto text-xs text-muted">{ws}</span>
      </header>

      {error ? (
        <div role="alert" className="bg-panel px-4 py-2 text-xs text-accent">
          {error === "denied" ? "You don't have access to this sidecar." : error}
        </div>
      ) : null}

      <div className="flex-1 px-4 py-3 text-sm">
        {status ? (
          <dl className="grid grid-cols-2 gap-1 text-xs">
            <dt className="text-muted">Version</dt>
            <dd>{status.version}</dd>
            <dt className="text-muted">Lifecycle</dt>
            <dd>{status.lifecycle}</dd>
            <dt className="text-muted">Running</dt>
            <dd>{status.running ? "yes" : "no"}</dd>
            <dt className="text-muted">Restarts</dt>
            <dd data-testid="restart-count">{status.restartCount}</dd>
          </dl>
        ) : (
          <p className="text-muted">Not installed on this node.</p>
        )}
      </div>

      <footer className="flex gap-2 border-t border-border px-4 py-3">
        <button
          type="button"
          onClick={() => void install()}
          className="rounded bg-accent px-3 py-1 text-xs text-bg"
        >
          Install
        </button>
        <button
          type="button"
          onClick={() => void restart()}
          disabled={!status?.running}
          className="rounded bg-panel px-3 py-1 text-xs disabled:opacity-50"
        >
          Restart
        </button>
        <button
          type="button"
          onClick={() => void stop()}
          disabled={!status?.running}
          className="rounded bg-panel px-3 py-1 text-xs disabled:opacity-50"
        >
          Stop
        </button>
      </footer>
    </section>
  );
}
