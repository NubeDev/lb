// A reusable node-side filesystem browser: navigate the HOST filesystem one directory level at a time
// (via `host.fs.list`) and pick a path that is resolved ON THE NODE, not in the browser. This is the
// shared engine behind two pickers with different "what's selectable" rules:
//   - "dir" mode  — the current folder is the candidate; a caller predicate says whether it's pickable
//                   (the Studio extension picker: a folder holding `extension.toml`).
//   - "file" mode — files are the candidates; a caller predicate says which files are pickable
//                   (the datasource sqlite `.db` file picker).
// It is anchored at a caller-supplied root and never browses above it (the fs tool would allow it, but
// the anchor is the UI contract). A manual path field is the fallback when the fs capability isn't
// granted. One responsibility: browse host dirs → hand back a selected path.

import { useEffect, useState } from "react";
import { ChevronRight, CircleCheck, File, Folder, FolderUp, Loader2, TriangleAlert } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { type HostFsEntry, type HostFsList, listHostDir } from "@/lib/host/fs.api";

export interface HostPathPickerProps {
  /** The current selection (a directory path in "dir" mode, a file path in "file" mode). */
  value: string;
  /** Set the selection — a valid path, or "" when the current view holds none. */
  onPick: (path: string) => void;
  /** Resolve the anchor root the browse is confined to; called once. */
  resolveRoot: () => Promise<string>;
  /** "dir": pick the current folder; "file": pick a file entry. */
  mode: "dir" | "file";
  /**
   * "dir" mode — true when the CURRENT folder is selectable (`list` is the folder's listing).
   * "file" mode — true when an ENTRY (a file) is selectable.
   */
  selectable: (arg: HostFsList | HostFsEntry) => boolean;
  /** Placeholder for the manual-path fallback input. */
  manualPlaceholder?: string;
  /**
   * When true (default) the anchor is a HARD WALL — the browse can't go above it (the extension
   * picker: paths outside the devkit root are rejected anyway). When false the anchor is only a
   * STARTING point — "Up" keeps working above it (the sqlite DB may live outside the home dir).
   */
  confineToRoot?: boolean;
  /** Optional banner renderer for "dir" mode (extension-picker guidance). Omit for "file" mode. */
  renderBanner?: (ctx: { ready: boolean; loading: boolean; folder: string }) => React.ReactNode;
}

const errMsg = (e: unknown) => (e instanceof Error ? e.message : String(e));
const join = (dir: string, name: string) => (dir.endsWith("/") ? dir + name : `${dir}/${name}`);
const parent = (dir: string) => {
  const i = dir.lastIndexOf("/");
  return i <= 0 ? dir : dir.slice(0, i);
};
const base = (p: string) => p.slice(p.lastIndexOf("/") + 1) || p;

export function HostPathPicker({
  value,
  onPick,
  resolveRoot,
  mode,
  selectable,
  manualPlaceholder,
  confineToRoot = true,
  renderBanner,
}: HostPathPickerProps) {
  const [root, setRoot] = useState<string | null>(null);
  const [cwd, setCwd] = useState("");
  const [list, setList] = useState<HostFsList | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Resolve the anchor once, then start there (or resume at `value`'s folder if it's under root).
  useEffect(() => {
    let alive = true;
    resolveRoot()
      .then((r) => {
        if (!alive) return;
        setRoot(r);
        const resume = value && value.startsWith(r) ? (mode === "file" ? parent(value) : value) : r;
        setCwd(resume);
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

  // List the current directory whenever it changes. In "dir" mode the folder itself may become the
  // selection; in "file" mode navigation never selects (only clicking a file does).
  useEffect(() => {
    if (!cwd) return;
    let alive = true;
    setLoading(true);
    setError(null);
    listHostDir(cwd)
      .then((l) => {
        if (!alive) return;
        setList(l);
        if (mode === "dir") onPick(selectable(l) ? l.path : "");
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
  }, [cwd, mode, onPick, selectable]);

  if (error && !root)
    return <ManualFallback value={value} onPick={onPick} error={error} placeholder={manualPlaceholder} />;

  // "Up" stops at the anchor when it's a hard wall; otherwise only at the true filesystem root.
  const atCeiling = confineToRoot ? !!root && cwd === root : cwd === parent(cwd);
  const ready = mode === "dir" && !!list && selectable(list);
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
            disabled={atCeiling || loading}
            onClick={() => setCwd(parent(cwd))}
            className="h-7 gap-1.5 px-2 text-muted"
          >
            <FolderUp size={14} />
            Up
          </Button>
          {loading && <Loader2 size={14} className="animate-spin text-muted" />}
          <span className="ml-auto truncate font-mono text-[11px] text-muted">{base(cwd) || "…"}</span>
        </div>

        <div aria-label="directory entries" className="max-h-64 min-h-[8rem] overflow-y-auto p-1">
          {error ? (
            <Row muted>
              <TriangleAlert size={14} className="shrink-0 text-amber-600 dark:text-amber-400" />
              <span className="truncate text-amber-700 dark:text-amber-300">{error}</span>
            </Row>
          ) : entries.length === 0 && !loading ? (
            <Row muted>This folder is empty.</Row>
          ) : (
            entries.map((e) => (
              <EntryRow
                key={e.name}
                entry={e}
                picked={mode === "file" && value === join(cwd, e.name)}
                canPick={mode === "file" && selectable(e)}
                onOpen={() => setCwd(join(cwd, e.name))}
                onSelect={() => onPick(join(cwd, e.name))}
              />
            ))
          )}
          {list?.truncated && <Row muted>Showing the first {list.entries.length} entries.</Row>}
        </div>
      </div>

      {renderBanner?.({ ready, loading, folder: base(cwd) })}
    </div>
  );
}

function dirsFirst(a: HostFsEntry, b: HostFsEntry): number {
  const rank = (e: HostFsEntry) => (e.kind === "dir" ? 0 : 1);
  return rank(a) - rank(b) || a.name.localeCompare(b.name);
}

function Breadcrumb({ root, cwd, onGo }: { root: string | null; cwd: string; onGo: (p: string) => void }) {
  if (!root) return <div className="h-6" />;
  // Prefer a compact root-relative trail; but if the browse walked ABOVE the anchor (unconfined
  // mode), fall back to the absolute path so the crumbs still point at real directories.
  const underRoot = cwd === root || cwd.startsWith(`${root}/`);
  const anchor = underRoot ? root : "/";
  const anchorLabel = underRoot ? base(root) : "/";
  const rel = cwd === anchor ? "" : cwd.slice(anchor.length).replace(/^\//, "");
  const parts = rel ? rel.split("/") : [];
  const seg = (i: number) => `${anchor === "/" ? "" : anchor}/${parts.slice(0, i + 1).join("/")}`;
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
      {crumb(anchorLabel, anchor, parts.length === 0)}
      {parts.map((p, i) => crumb(p, seg(i), i === parts.length - 1))}
    </div>
  );
}

function EntryRow({
  entry,
  picked,
  canPick,
  onOpen,
  onSelect,
}: {
  entry: HostFsEntry;
  picked: boolean;
  canPick: boolean;
  onOpen: () => void;
  onSelect: () => void;
}) {
  if (entry.kind === "dir") {
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
  // A pickable file is a button; a non-pickable file is a muted static row.
  if (canPick) {
    return (
      <Button
        type="button"
        variant="ghost"
        size="sm"
        onClick={onSelect}
        className={cn(
          "h-8 w-full justify-start gap-2 px-2 font-normal",
          picked && "bg-accent/10 text-accent",
        )}
      >
        <File size={14} className={cn("shrink-0", picked ? "text-accent" : "text-fg/80")} />
        <span className="truncate">{entry.name}</span>
        {picked && <CircleCheck size={14} className="ml-auto shrink-0 text-accent" />}
      </Button>
    );
  }
  return (
    <Row muted>
      <File size={14} className="shrink-0 text-muted/70" />
      <span className="truncate">{entry.name}</span>
    </Row>
  );
}

function Row({ children, muted }: { children: React.ReactNode; muted?: boolean }) {
  return (
    <div className={cn("flex items-center gap-2 px-2 py-1.5 text-xs", muted ? "text-muted" : "text-fg")}>
      {children}
    </div>
  );
}

function ManualFallback({
  value,
  onPick,
  error,
  placeholder,
}: {
  value: string;
  onPick: (path: string) => void;
  error: string;
  placeholder?: string;
}) {
  return (
    <div className="flex flex-col gap-1.5">
      <Input
        value={value}
        onChange={(e) => onPick(e.target.value)}
        placeholder={placeholder ?? "path on the node"}
        className="font-mono"
        spellCheck={false}
      />
      <span className="flex items-center gap-1.5 text-[11px] text-muted">
        <TriangleAlert size={12} className="shrink-0 text-amber-600 dark:text-amber-400" />
        Browsing unavailable ({error}). Type the path instead.
      </span>
    </div>
  );
}
