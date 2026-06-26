// The registry view — browse an extension's catalog, install a version, or roll back to a prior one.
// Layout + wiring only; data lives in useRegistry (FILE-LAYOUT). This is the UI face of the S7 story:
// installing a SIGNED version succeeds and becomes the live install; installing one that fails
// verification is REFUSED ("artifact failed verification"); a user without the grant is denied — the
// same gates the Rust registry tests prove on the backend, surfaced to the user. Rollback is the same
// Install button on a prior version (registry scope: rollback is pulling the previous version).

import { Package } from "lucide-react";

import { useRegistry } from "./useRegistry";

interface Props {
  ws: string;
  extId: string;
  /** The current user's principal (demo session identity until real login lands). */
  author: string;
  /** The caller's held capabilities (the grant the node checks; demo until real tokens). */
  caps: string[];
}

export function RegistryView({ ws, extId, author, caps }: Props) {
  const { entries, installedVersion, unverified, error, install } = useRegistry(
    ws,
    extId,
    author,
    caps,
  );

  return (
    <section className="flex h-full flex-col bg-bg">
      <header className="flex items-center gap-2 border-b border-border px-4 py-3">
        <Package size={16} className="text-muted" />
        <h1 className="text-sm font-medium">Extension registry — {extId}</h1>
        <span className="ml-auto text-xs text-muted">{ws}</span>
      </header>

      {error ? (
        <div role="alert" className="bg-panel px-4 py-2 text-xs text-accent">
          {error === "denied" ? "You don't have access to the registry." : error}
        </div>
      ) : unverified ? (
        <div role="alert" className="bg-panel px-4 py-2 text-xs text-accent">
          Artifact failed verification — refused. Nothing was installed.
        </div>
      ) : installedVersion ? (
        <div role="status" className="bg-panel px-4 py-2 text-xs text-accent">
          Installed {extId}@{installedVersion}.
        </div>
      ) : null}

      {entries.length > 0 ? (
        <ul className="flex-1 px-4 py-3 text-sm">
          {entries.map((e) => (
            <li key={e.version} className="flex items-center gap-2 py-1">
              <span>
                {e.extId}@{e.version}
              </span>
              <span className="text-xs text-muted">({e.visibility})</span>
              {installedVersion === e.version ? (
                <span className="text-xs text-accent">installed</span>
              ) : null}
              <button
                type="button"
                onClick={() => void install(e.version)}
                className="ml-auto rounded bg-accent px-3 py-1 text-xs text-bg"
              >
                {installedVersion && installedVersion !== e.version ? "Roll back to" : "Install"}{" "}
                {e.version}
              </button>
            </li>
          ))}
        </ul>
      ) : (
        <div className="flex flex-1 items-center justify-center text-sm text-muted">
          No versions in the catalog.
        </div>
      )}
    </section>
  );
}
