// The "open existing" folder browser — so a user picks an extension by navigating to it instead of
// typing a path (the old dead-end). It browses the host filesystem one level at a time via
// `host.fs.list`, anchored at the devkit root (`devkit.root`) since that's the only tree `inspect`/
// `build`/`publish` accept a path under — you can't (and needn't) go above it. The current folder is
// the candidate: when it holds an `extension.toml`, it's selectable; otherwise the picker guides the
// user deeper. A manual path field is the fallback when the fs capability isn't granted.

import { useEffect, useState } from "react";
import { ChevronRight, CircleCheck, File, Folder, FolderUp, Loader2, TriangleAlert } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { devkitRoot } from "@/lib/devkit/devkit.api";
import { type HostFsEntry, type HostFsList, listHostDir } from "@/lib/host/fs.api";

interface Props {
  /** The currently-selected folder (the wizard's path). */
  value: string;
  /** Set the selection — a valid extension folder, or "" while the current folder isn't one. */
  onPick: (path: string) => void;
}

const errMsg = (e: unknown) => (e instanceof Error ? e.message : String(e));
const join = (dir: string, name: string) => (dir.endsWith("/") ? dir + name : `${dir}/${name}`);
const parent = (dir: string) => {
  const i = dir.lastIndexOf("/");
  return i <= 0 ? dir : dir.slice(0, i);
};
const base = (p: string) => p.slice(p.lastIndexOf("/") + 1) || p;

export function DirectoryPicker({ value, onPick }: Props) {
  const [root, setRoot] = useState<string | null>(null);
  const [cwd, setCwd] = useState("");
  const [list, setList] = useState<HostFsList | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Resolve the devkit root once, then anchor there (or resume at `value` if it's under root).
  useEffect(() => {
    let alive = true;
    devkitRoot()
      .then((r) => {
        if (!alive) return;
        setRoot(r.path);
        setCwd(value && value.startsWith(r.path) ? value : r.path);
      })
      .catch((e) => {
        if (!alive) return;
        setError(errMsg(e));
        setLoading(false);
      });
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- resolve the anchor exactly once
  }, []);

  // List the current directory whenever it changes; a folder holding extension.toml is selectable.
  useEffect(() => {
    if (!cwd) return;
    let alive = true;
    setLoading(true);
    setError(null);
    listHostDir(cwd)
      .then((l) => {
        if (!alive) return;
        setList(l);
        onPick(isExtension(l) ? l.path : "");
        setLoading(false);
      })
      .catch((e) => {
        if (!alive) return;
        setError(errMsg(e));
        setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [cwd, onPick]);

  if (error && !root) return <ManualFallback value={value} onPick={onPick} error={error} />;

  const atRoot = !!root && cwd === root;
  const ready = !!list && isExtension(list);
  const entries = list ? [...list.entries].sort(dirsFirst) : [];

  return (
    <div className="flex min-h-0 flex-col gap-2">
      <Breadcrumb root={root} cwd={cwd} onGo={setCwd} />

      <div className="min-h-0 overflow-hidden rounded-md border border-border bg-bg">
        <div className="flex items-center gap-2 border-b border-border bg-panel/40 px-2 py-1.5">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            disabled={atRoot || loading}
            onClick={() => setCwd(parent(cwd))}
            className="h-7 gap-1.5 px-2 text-muted"
          >
            <FolderUp size={14} />
            Up
          </Button>
          {loading && <Loader2 size={14} className="animate-spin text-muted" />}
          <span className="ml-auto truncate font-mono text-[11px] text-muted">{base(cwd) || "…"}</span>
        </div>

        <div className="max-h-64 min-h-[8rem] overflow-y-auto p-1">
          {error ? (
            <Row muted>
              <TriangleAlert size={14} className="shrink-0 text-amber-600 dark:text-amber-400" />
              <span className="truncate text-amber-700 dark:text-amber-300">{error}</span>
            </Row>
          ) : entries.length === 0 && !loading ? (
            <Row muted>This folder is empty.</Row>
          ) : (
            entries.map((e) => (
              <EntryRow key={e.name} entry={e} onOpen={() => setCwd(join(cwd, e.name))} />
            ))
          )}
          {list?.truncated && <Row muted>Showing the first {list.entries.length} entries.</Row>}
        </div>
      </div>

      <Banner ready={ready} loading={loading} folder={base(cwd)} />
    </div>
  );
}

function isExtension(list: HostFsList): boolean {
  return list.entries.some((e) => e.name === "extension.toml" && e.kind === "file");
}

function dirsFirst(a: HostFsEntry, b: HostFsEntry): number {
  const rank = (e: HostFsEntry) => (e.kind === "dir" ? 0 : 1);
  return rank(a) - rank(b) || a.name.localeCompare(b.name);
}

function Breadcrumb({ root, cwd, onGo }: { root: string | null; cwd: string; onGo: (p: string) => void }) {
  if (!root) return <div className="h-6" />;
  const rel = cwd === root ? "" : cwd.slice(root.length).replace(/^\//, "");
  const parts = rel ? rel.split("/") : [];
  const crumb = (label: string, target: string, last: boolean) => (
    <div key={target} className="flex min-w-0 items-center">
      <Button
        type="button"
        variant="ghost"
        size="sm"
        disabled={last}
        onClick={() => onGo(target)}
        className={cn(
          "h-6 max-w-[12rem] truncate px-1.5 font-mono text-[11px]",
          last ? "text-fg disabled:opacity-100" : "text-muted",
        )}
      >
        {label}
      </Button>
      {!last && <ChevronRight size={12} className="shrink-0 text-muted/60" />}
    </div>
  );
  return (
    <div className="flex flex-wrap items-center">
      {crumb(base(root), root, parts.length === 0)}
      {parts.map((p, i) =>
        crumb(p, `${root}/${parts.slice(0, i + 1).join("/")}`, i === parts.length - 1),
      )}
    </div>
  );
}

function EntryRow({ entry, onOpen }: { entry: HostFsEntry; onOpen: () => void }) {
  const isDir = entry.kind === "dir";
  const isManifest = entry.name === "extension.toml" && entry.kind === "file";
  if (!isDir) {
    return (
      <Row muted={!isManifest}>
        <File size={14} className={cn("shrink-0", isManifest ? "text-accent" : "text-muted/70")} />
        <span className={cn("truncate", isManifest && "font-medium text-accent")}>{entry.name}</span>
        {isManifest && <span className="ml-auto text-[11px] text-accent">extension manifest</span>}
      </Row>
    );
  }
  return (
    <Button
      type="button"
      variant="ghost"
      size="sm"
      onClick={onOpen}
      className="h-8 w-full justify-start gap-2 px-2 font-normal"
    >
      <Folder size={14} className="shrink-0 text-accent/80" />
      <span className="truncate">{entry.name}</span>
      <ChevronRight size={14} className="ml-auto shrink-0 text-muted/50" />
    </Button>
  );
}

function Row({ children, muted }: { children: React.ReactNode; muted?: boolean }) {
  return (
    <div
      className={cn(
        "flex items-center gap-2 px-2 py-1.5 text-xs",
        muted ? "text-muted" : "text-fg",
      )}
    >
      {children}
    </div>
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

function ManualFallback({
  value,
  onPick,
  error,
}: {
  value: string;
  onPick: (path: string) => void;
  error: string;
}) {
  return (
    <div className="flex flex-col gap-1.5">
      <Input
        value={value}
        onChange={(e) => onPick(e.target.value)}
        placeholder="rust/extensions/my-extension"
        className="font-mono"
        spellCheck={false}
      />
      <span className="flex items-center gap-1.5 text-[11px] text-muted">
        <TriangleAlert size={12} className="shrink-0 text-amber-600 dark:text-amber-400" />
        Browsing unavailable ({error}). Type the folder path instead.
      </span>
    </div>
  );
}
