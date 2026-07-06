// The open-query dialog (datasources-ux + query scope) — lists the saved queries that target THIS
// datasource (the hook already filtered the roster) and loads the chosen one into the SQL editor on
// click. Mirrors the `AddDatasourceDialog` action-in-header pattern. A per-row delete (tombstone)
// keeps the list tidy. The dialog never runs a query — opening loads the text; the author hits Run.
// One responsibility, one file (FILE-LAYOUT).

import { useState } from "react";
import { FolderOpen, Loader2, Trash2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { QuerySummary } from "@/lib/queries";

interface Props {
  /** Saved queries already filtered to this datasource (the parent's hook owns the filter). */
  queries: QuerySummary[];
  loading: boolean;
  error: string | null;
  /** Load the chosen query's text into the editor (the author hits Run). */
  onLoad: (query: QuerySummary) => void;
  /** Soft-delete a saved query (idempotent tombstone). */
  onDelete: (id: string) => Promise<void>;
}

export function SavedQueriesDialog({ queries, loading, error, onLoad, onDelete }: Props) {
  const [open, setOpen] = useState(false);
  const [removing, setRemoving] = useState<string | null>(null);

  const choose = (q: QuerySummary) => {
    onLoad(q);
    setOpen(false);
  };

  const remove = async (id: string) => {
    setRemoving(id);
    try {
      await onDelete(id);
    } finally {
      setRemoving(null);
    }
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <Button
        aria-label="open saved query"
        size="sm"
        variant="ghost"
        className="gap-1.5"
        onClick={() => setOpen(true)}
      >
        <FolderOpen size={13} /> Open
      </Button>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Saved queries</DialogTitle>
          <DialogDescription>
            Saved SQL against this datasource. Click a row to load it into the editor.
          </DialogDescription>
        </DialogHeader>
        <div aria-label="saved query list" className="max-h-80 overflow-y-auto">
          {loading && (
            <div className="flex items-center justify-center gap-2 py-6 text-sm text-muted">
              <Loader2 size={14} className="animate-spin" /> loading…
            </div>
          )}
          {!loading && error && (
            <p role="alert" className="px-1 py-4 text-center text-sm text-destructive">
              {error}
            </p>
          )}
          {!loading && !error && queries.length === 0 && (
            <p className="px-1 py-6 text-center text-sm text-muted">
              No saved queries against this datasource yet.
            </p>
          )}
          {!loading && !error && queries.length > 0 && (
            <ul className="space-y-1">
              {queries.map((q) => (
                <li
                  key={q.id}
                  className="flex items-center gap-2 rounded-md border border-border bg-bg px-2 py-1.5"
                >
                  <button
                    type="button"
                    aria-label={`open ${q.name || q.id}`}
                    className="flex min-w-0 flex-1 flex-col items-start gap-0.5 text-left"
                    onClick={() => choose(q)}
                  >
                    <span className="truncate text-sm font-medium">{q.name || q.id}</span>
                    <span className="truncate font-mono text-[11px] text-muted">{q.id}</span>
                  </button>
                  <Button
                    aria-label={`delete ${q.id}`}
                    size="sm"
                    variant="ghost"
                    className="h-7 shrink-0 px-1.5 text-muted hover:text-destructive"
                    disabled={removing === q.id}
                    onClick={() => void remove(q.id)}
                  >
                    {removing === q.id ? (
                      <Loader2 size={13} className="animate-spin" />
                    ) : (
                      <Trash2 size={13} />
                    )}
                  </Button>
                </li>
              ))}
            </ul>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
