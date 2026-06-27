// The unified extensions console (admin-console + lifecycle-management scopes) — the real console
// that SUPERSEDES the demo RegistryView + NativeView. Lists installed extensions across BOTH tiers
// (wasm/native) with tier · version · enabled · running · health · restart count; start/stop via
// enable/disable; uninstall (binary eviction). Install-from-catalog / upload is optional this pass
// (deferred — the scope allows it). Every disable/uninstall routes through the shared
// ConfirmDestructive. Markup + wiring only; data lives in useExtensions. The gateway re-checks every
// verb; the UI gate is convenience, not the boundary.

import { useState } from "react";
import { Boxes } from "lucide-react";

import { ConfirmDestructive } from "@/features/confirm";
import { useExtensions } from "./useExtensions";
import { UploadArtifact } from "./UploadArtifact";

type Pending = { kind: "disable" | "uninstall"; ext: string } | null;

interface Props {
  ws: string;
}

export function ExtensionsView({ ws }: Props) {
  const { rows, error, setEnabled, uninstall, upload } = useExtensions();
  const [pending, setPending] = useState<Pending>(null);

  return (
    <section className="flex h-full flex-col bg-bg">
      <header className="page-header">
        <div className="page-header-icon">
          <Boxes size={16} />
        </div>
        <div className="min-w-0">
          <h1 className="page-title">Extensions</h1>
          <p className="page-subtitle">Installed extension tiers, health, and lifecycle actions.</p>
        </div>
        <div className="ml-auto flex items-center gap-3">
          <UploadArtifact onUpload={upload} />
          <span className="scope-pill" title={`Workspace ${ws}`}>
            <span className="h-1.5 w-1.5 rounded-full bg-accent" aria-hidden />
            <span className="truncate">{ws}</span>
          </span>
        </div>
      </header>

      {error && (
        <div role="alert" className="state-alert">
          {error}
        </div>
      )}

      <ul className="flex-1 overflow-y-auto px-4 py-2">
        {rows.length === 0 ? (
          <li className="text-sm text-muted">No extensions installed.</li>
        ) : (
          rows.map((r) => (
            <li key={r.ext} className="flex items-center gap-2 py-1 text-sm" role="listitem">
              <span>
                {r.ext}@{r.version}
              </span>
              <span className="rounded bg-panel px-1.5 py-0.5 text-xs text-muted">{r.tier}</span>
              <span className={`text-xs ${r.running ? "text-accent" : "text-muted"}`}>
                {r.health}
              </span>
              {r.tier === "native" && (
                <span className="text-xs text-muted" data-testid={`restarts-${r.ext}`}>
                  restarts {r.restart_count}
                </span>
              )}
              <button
                aria-label={`${r.enabled ? "stop" : "start"} ${r.ext}`}
                className="ml-auto rounded bg-panel px-2 py-0.5 text-xs"
                onClick={() =>
                  r.enabled ? setPending({ kind: "disable", ext: r.ext }) : void setEnabled(r.ext, true)
                }
              >
                {r.enabled ? "Stop" : "Start"}
              </button>
              <button
                aria-label={`uninstall ${r.ext}`}
                className="rounded bg-red-500/15 px-2 py-0.5 text-xs text-red-400"
                onClick={() => setPending({ kind: "uninstall", ext: r.ext })}
              >
                Uninstall
              </button>
            </li>
          ))
        )}
      </ul>

      {pending?.kind === "disable" && (
        <ConfirmDestructive
          title={`Stop ${pending.ext}`}
          consequence={`Disables ${pending.ext}: a running native child is stopped and the boot reconciler will NOT auto-start it. The install stays — re-enable to start again. Reversible.`}
          reversible
          escalation="none"
          confirmLabel="Stop"
          onConfirm={() => {
            void setEnabled(pending.ext, false);
            setPending(null);
          }}
          onCancel={() => setPending(null)}
        />
      )}
      {pending?.kind === "uninstall" && (
        <ConfirmDestructive
          title={`Uninstall ${pending.ext}`}
          consequence={`Stops any running child, tombstones the install record, and evicts the cached binary. The extension disappears from this workspace; reinstall from the registry to restore it.`}
          reversible={false}
          escalation="second-gate"
          confirmLabel="Uninstall"
          onConfirm={() => {
            void uninstall(pending.ext);
            setPending(null);
          }}
          onCancel={() => setPending(null)}
        />
      )}
    </section>
  );
}
