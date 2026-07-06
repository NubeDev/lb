// The open-query dialog (datasources-ux + query scope) — lists the saved queries that target THIS
// datasource (the hook already filtered the roster) and exposes three row actions:
//   - Click the row body → load its text into the editor (the author hits Run).
//   - Expand chevron → lazy-load the text and render it inline in a read-only `SqlEditor` (the SAME
//     CodeMirror component the datasource's Code mode uses — syntax-highlighted "view it nice",
//     not a flat `<pre>`). One fetch per row, cached in dialog state for the session.
//   - Copy button → lazy-load the text and write it to the clipboard (transient "Copied ✓").
//   - Per-row delete (tombstone) keeps the list tidy.
//
// The dialog never runs a query — opening loads the text; the author hits Run. The summary roster
// (`query.list`) carries no text, so copy + expand lazy-fetch via `onFetchText` (the parent hooks the
// shipped `query.get`). Mirrors the `AddDatasourceDialog` action-in-header pattern. One
// responsibility, one file (FILE-LAYOUT).

import { useCallback, useState } from "react";
import {
  Check,
  ChevronDown,
  ChevronRight,
  ClipboardCopy,
  FolderOpen,
  Loader2,
  Trash2,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { SqlEditor } from "@/features/dashboard/builder/editors/SqlEditor";
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
  /** Fetch one saved query's SQL text on demand (`query.get` → `.text`). Used by copy + expand —
   *  the roster summary carries no text, so they lazy-load on first interaction. */
  onFetchText: (id: string) => Promise<string>;
}

export function SavedQueriesDialog({
  queries,
  loading,
  error,
  onLoad,
  onDelete,
  onFetchText,
}: Props) {
  const [open, setOpen] = useState(false);
  const [removing, setRemoving] = useState<string | null>(null);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [fetching, setFetching] = useState<string | null>(null);
  const [textCache, setTextCache] = useState<Record<string, string>>({});
  const [copiedId, setCopiedId] = useState<string | null>(null);

  /** Lazy-load one query's text on first need (copy or expand). Cached per dialog session so a
   *  second interaction on the same row is instant and only ONE `query.get` fires per row. A load
   *  failure (deny/NotFound) returns null — the caller no-ops, no fabricated text. */
  const ensureText = useCallback(
    async (id: string): Promise<string | null> => {
      if (textCache[id] !== undefined) return textCache[id];
      setFetching(id);
      try {
        const text = await onFetchText(id);
        setTextCache((c) => ({ ...c, [id]: text }));
        return text;
      } catch {
        return null;
      } finally {
        setFetching(null);
      }
    },
    [textCache, onFetchText],
  );

  const choose = (q: QuerySummary) => {
    onLoad(q);
    setOpen(false);
  };

  const toggleExpand = async (q: QuerySummary) => {
    const next = expandedId === q.id ? null : q.id;
    setExpandedId(next);
    if (next !== null && textCache[q.id] === undefined) {
      await ensureText(q.id);
    }
  };

  const copy = async (q: QuerySummary) => {
    const text = await ensureText(q.id);
    if (text == null) return;
    try {
      await navigator.clipboard?.writeText(text);
      setCopiedId(q.id);
      setTimeout(() => setCopiedId((c) => (c === q.id ? null : c)), 1500);
    } catch {
      // Clipboard denied (permissions / insecure context) — leave the button unchanged rather than
      // erroring; the user can retry. Nothing destructive happened.
    }
  };

  const remove = async (id: string) => {
    setRemoving(id);
    try {
      await onDelete(id);
      // Drop the row's cached text + collapse it if it was open (the row is going away regardless).
      setTextCache((c) => {
        const next = { ...c };
        delete next[id];
        return next;
      });
      if (expandedId === id) setExpandedId(null);
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
            Saved SQL against this datasource. Click a row to load it into the editor, copy the SQL,
            or expand to view it.
          </DialogDescription>
        </DialogHeader>
        <div aria-label="saved query list" className="max-h-[60vh] overflow-y-auto">
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
              {queries.map((q) => {
                const isOpen = expandedId === q.id;
                const text = textCache[q.id];
                return (
                  <li
                    key={q.id}
                    className="overflow-hidden rounded-md border border-border bg-bg"
                  >
                    <div className="flex items-center gap-1 px-2 py-1.5">
                      <Button
                        aria-label={isOpen ? `collapse ${q.name || q.id}` : `expand ${q.name || q.id}`}
                        aria-expanded={isOpen}
                        title={isOpen ? "Collapse" : "View query"}
                        size="sm"
                        variant="ghost"
                        className="h-7 shrink-0 px-1.5 text-muted"
                        disabled={fetching === q.id}
                        onClick={() => void toggleExpand(q)}
                      >
                        {fetching === q.id ? (
                          <Loader2 size={13} className="animate-spin" />
                        ) : isOpen ? (
                          <ChevronDown size={13} />
                        ) : (
                          <ChevronRight size={13} />
                        )}
                      </Button>
                      {/* eslint-disable-next-line no-restricted-syntax -- a row body select, not a Button */}
                      <button
                        type="button"
                        aria-label={`open ${q.name || q.id}`}
                        className="flex min-w-0 flex-1 flex-col items-start gap-0.5 rounded px-1 py-0.5 text-left hover:bg-panel/40"
                        onClick={() => choose(q)}
                      >
                        <span className="truncate text-sm font-medium">{q.name || q.id}</span>
                        <span className="truncate font-mono text-[11px] text-muted">{q.id}</span>
                      </button>
                      <Button
                        aria-label={`copy ${q.name || q.id}`}
                        title="Copy SQL to clipboard"
                        size="sm"
                        variant="ghost"
                        className="h-7 shrink-0 px-1.5 text-muted"
                        disabled={fetching === q.id}
                        onClick={() => void copy(q)}
                      >
                        {copiedId === q.id ? (
                          <Check size={13} className="text-accent" />
                        ) : (
                          <ClipboardCopy size={13} />
                        )}
                      </Button>
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
                    </div>
                    {isOpen && text !== undefined && (
                      <div
                        aria-label={`saved query text ${q.id}`}
                        className="border-t border-border bg-panel/20 px-2 pb-2 pt-1.5"
                      >
                        {/* The SAME CodeMirror `SqlEditor` the datasource's Code mode uses
                            (slice 2 of the query-builder scope) — read-only here so "view it nice"
                            means real syntax highlighting, not a flat `<pre>`. Federation saved
                            queries are always `lang:"raw"` SQL → standard dialect (rule 10: derived
                            from the row's target kind, never a hardcoded name). */}
                        <SqlEditor
                          value={text}
                          onChange={() => {
                            /* read-only view — edits happen in the workbench after Load */
                          }}
                          editable={false}
                          dialect="standard"
                          height="160px"
                        />
                      </div>
                    )}
                  </li>
                );
              })}
            </ul>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
