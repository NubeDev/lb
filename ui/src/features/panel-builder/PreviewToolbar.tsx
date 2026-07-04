// The preview toolbar (data-studio-ux scope, "Run semantics"). It owns the actions that used to be
// implicit or scattered: a real Run/Refresh button for EVERY datasource (not just federation), a Freeze
// toggle ("use current data" — edit-without-requery), and the table-view inspect toggle. Save/"Save to
// tab" is NOT here — that persists the draft; making it look like a run button was the core confusion.
//
// One responsibility: the preview action row. State lives in BuilderPane; this is a pure view.

import { RefreshCw, Snowflake, Table2 } from "lucide-react";

import { Button } from "@/components/ui/button";

interface Props {
  /** Whether the draft has a resolvable source (Run is meaningless without one). */
  hasTarget: boolean;
  /** A query is in flight — the Run button shows a spinning icon and is disabled. */
  loading: boolean;
  /** Preview is frozen (datasource not re-hit; edits reshape cached frames). */
  frozen: boolean;
  /** Force a fresh query even for a byte-identical spec (the editor's `run` nonce). */
  onRun: () => void;
  /** Toggle freeze. */
  onToggleFreeze: () => void;
  /** Table-view inspect state + toggle (render the frames as a grid regardless of the chosen viz). */
  tableView: boolean;
  onToggleTableView: () => void;
}

export function PreviewToolbar({
  hasTarget,
  loading,
  frozen,
  onRun,
  onToggleFreeze,
  tableView,
  onToggleTableView,
}: Props) {
  return (
    <div className="flex items-center gap-1.5" aria-label="preview toolbar">
      <Button
        type="button"
        size="sm"
        variant="default"
        aria-label="run query"
        className="h-7 px-2 text-xs"
        disabled={!hasTarget || loading || frozen}
        title={frozen ? "Unfreeze to run a fresh query" : "Run the query (⌘/Ctrl+Enter)"}
        onClick={onRun}
      >
        <RefreshCw size={12} className={loading ? "animate-spin" : undefined} /> Run
      </Button>
      <Button
        type="button"
        size="sm"
        variant={frozen ? "default" : "ghost"}
        aria-label="freeze preview data"
        aria-pressed={frozen}
        className="h-7 px-2 text-xs"
        title={
          frozen
            ? "Frozen — edits reshape the data already fetched. Click to unfreeze and re-query."
            : "Freeze the current data — iterate on the chart without re-querying the datasource."
        }
        onClick={onToggleFreeze}
      >
        <Snowflake size={12} /> {frozen ? "Frozen" : "Freeze"}
      </Button>
      <span className="flex-1" />
      <Button
        type="button"
        size="sm"
        variant={tableView ? "default" : "ghost"}
        aria-label="toggle table view"
        aria-pressed={tableView}
        className="h-7 px-2 text-xs"
        title="Inspect the transformed frames as a table"
        onClick={onToggleTableView}
      >
        <Table2 size={12} /> Table view
      </Button>
    </div>
  );
}
