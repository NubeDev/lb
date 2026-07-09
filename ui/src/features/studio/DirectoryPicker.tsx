// The "open existing" folder browser — so a user picks an extension by navigating to it instead of
// typing a path (the old dead-end). It is the shared `HostPathPicker` engine in "dir" mode, anchored
// at the devkit root (`devkit.root`) since that's the only tree `inspect`/`build`/`publish` accept a
// path under. A folder is selectable when it holds an `extension.toml`; otherwise the banner guides the
// user deeper. All the browse/breadcrumb/fallback mechanics live in `HostPathPicker`; this file only
// supplies the extension-specific rules (root, selectable predicate, guidance banner).

import { CircleCheck, Folder } from "lucide-react";

import { cn } from "@/lib/utils";
import { devkitRoot } from "@/lib/devkit/devkit.api";
import { type HostFsEntry, type HostFsList } from "@/lib/host/fs.api";
import { HostPathPicker } from "@/lib/host/HostPathPicker";

interface Props {
  /** The currently-selected folder (the wizard's path). */
  value: string;
  /** Set the selection — a valid extension folder, or "" while the current folder isn't one. */
  onPick: (path: string) => void;
}

/** A folder is a candidate when it holds an `extension.toml`. */
function isExtension(arg: HostFsList | HostFsEntry): boolean {
  const list = arg as HostFsList;
  return (
    Array.isArray(list.entries) &&
    list.entries.some((e) => e.name === "extension.toml" && e.kind === "file")
  );
}

export function DirectoryPicker({ value, onPick }: Props) {
  return (
    <HostPathPicker
      value={value}
      onPick={onPick}
      mode="dir"
      resolveRoot={() => devkitRoot().then((r) => r.path)}
      selectable={isExtension}
      manualPlaceholder="rust/extensions/my-extension"
      renderBanner={({ ready, loading, folder }) => <Banner ready={ready} loading={loading} folder={folder} />}
    />
  );
}

function Banner({ ready, loading, folder }: { ready: boolean; loading: boolean; folder: string }) {
  if (loading) return <div className="h-8" />;
  return (
    <div
      className={cn(
        "flex items-center gap-2 rounded-md border px-3 py-2 text-xs",
        ready
          ? "border-accent/25 bg-accent/10 text-fg"
          : "border-border bg-panel/50 text-muted",
      )}
    >
      {ready ? (
        <>
          <CircleCheck size={14} className="shrink-0 text-accent" />
          <span className="min-w-0 truncate">
            <span className="font-mono text-fg">{folder}</span> is an extension — open it below.
          </span>
        </>
      ) : (
        <>
          <Folder size={14} className="shrink-0" />
          <span className="min-w-0">
            No <span className="font-mono">extension.toml</span> here — open a subfolder to find one.
          </span>
        </>
      )}
    </div>
  );
}
