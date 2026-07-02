// A one-click MRU list at the top of the Create step: the last few extensions the user opened/built,
// remembered in localStorage. Clicking a row skips the whole pick-a-path flow and goes straight to
// Build. Empty when nothing's been built yet, so first-run users never see it.

import { History, Play, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import type { RecentExtension } from "@/lib/devkit/recent-extensions";

export function RecentBuilds({
  recent,
  busy,
  onRun,
  onForget,
}: {
  recent: RecentExtension[];
  busy: boolean;
  onRun: (path: string) => void;
  onForget: (path: string) => void;
}) {
  if (recent.length === 0) return null;

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center gap-1.5 text-xs font-medium text-muted">
        <History size={13} />
        Recent — click to rebuild
      </div>
      <div className="grid gap-1.5">
        {recent.map((r) => (
          <RecentRow
            key={r.path}
            entry={r}
            busy={busy}
            onRun={() => onRun(r.path)}
            onForget={() => onForget(r.path)}
          />
        ))}
      </div>
    </div>
  );
}

function RecentRow({
  entry,
  busy,
  onRun,
  onForget,
}: {
  entry: RecentExtension;
  busy: boolean;
  onRun: () => void;
  onForget: () => void;
}) {
  return (
    <div className="group flex items-center gap-2 rounded-lg border border-border bg-panel/40 px-2.5 py-2 hover:border-accent/40">
      <button
        type="button"
        disabled={busy}
        onClick={onRun}
        className="flex min-w-0 flex-1 items-center gap-2.5 text-left disabled:opacity-50"
      >
        <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md border border-accent/30 bg-accent/15 text-accent">
          <Play size={13} />
        </span>
        <span className="min-w-0">
          <span className="flex items-center gap-1.5">
            <span className="truncate font-mono text-xs font-medium text-fg">
              {entry.id}
            </span>
            <span className="shrink-0 rounded bg-border/50 px-1.5 py-0 text-[10px] text-muted">
              {entry.tier}
            </span>
            <span className="shrink-0 text-[10px] text-muted">
              {timeAgo(entry.at)}
            </span>
          </span>
          <span className="mt-0.5 block truncate text-[10px] text-muted">
            {entry.path}
          </span>
        </span>
      </button>
      <Button
        type="button"
        variant="ghost"
        size="icon"
        aria-label={`Forget ${entry.id}`}
        onClick={onForget}
        className="h-6 w-6 shrink-0 text-muted opacity-0 transition-opacity hover:text-fg group-hover:opacity-100"
      >
        <X size={13} />
      </Button>
    </div>
  );
}

function timeAgo(epochMs: number): string {
  const secs = Math.max(0, Math.floor((Date.now() - epochMs) / 1000));
  if (secs < 60) return "just now";
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  return `${Math.floor(hours / 24)}d ago`;
}
