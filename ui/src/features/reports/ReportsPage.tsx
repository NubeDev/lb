// ReportsPage — the reports roster (reports scope, the dashboards-manager pattern). Lists every report
// the caller can reach (own + team-shared + workspace), with New report / open / delete. Every mutation
// funnels through the shipped `report.*` verbs, each capability-gated + workspace-walled server-side.
// Opening a report hands control to the editor (via `onOpen`); this page is the LIBRARY surface. One
// responsibility: the roster table + its toolbar.

import { useEffect, useMemo, useState } from "react";
import { FileText, Plus, Trash2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { AppEmptyState } from "@/components/app/empty-state";
import { ConfirmDestructive } from "@/features/confirm/ConfirmDestructive";
import { deleteReport, listReports, saveReport, type ReportSummary } from "@/lib/report";
import { slugFromTitle } from "@/lib/dashboard";

interface Props {
  ws: string;
  onOpen: (id: string) => void;
}

export function ReportsPage({ onOpen }: Props) {
  const [reports, setReports] = useState<ReportSummary[]>([]);
  const [query, setQuery] = useState("");
  const [creating, setCreating] = useState(false);
  const [newTitle, setNewTitle] = useState("");
  const [pendingDelete, setPendingDelete] = useState<ReportSummary | null>(null);
  const [error, setError] = useState<string | undefined>();

  async function refresh() {
    try {
      setReports(await listReports());
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  const rows = useMemo(() => {
    const q = query.trim().toLowerCase();
    const list = q ? reports.filter((r) => r.title.toLowerCase().includes(q) || bare(r.id).includes(q)) : reports;
    return [...list].sort((a, b) => b.updated_ts - a.updated_ts);
  }, [reports, query]);

  async function create() {
    const t = newTitle.trim();
    if (!t) return;
    const taken = new Set(reports.map((r) => bare(r.id)));
    let id = slugFromTitle(t);
    let n = 2;
    while (taken.has(id)) id = `${slugFromTitle(t)}-${n++}`;
    try {
      // A fresh empty report — no brandId forces the host default; the editor's BrandPicker fills it.
      await saveReport(id, t, [], "", { range: {} });
      setNewTitle("");
      setCreating(false);
      await refresh();
      onOpen(id);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  return (
    <>
      <div className="flex items-center gap-2 border-b border-border px-4 py-2">
        <Input
          aria-label="filter reports"
          className="h-8 w-64 text-xs"
          placeholder="Filter by title or id…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <span className="text-xs text-muted">
          {rows.length} report{rows.length === 1 ? "" : "s"}
        </span>
        <div className="ml-auto">
          <Button size="sm" onClick={() => setCreating(true)}>
            <Plus size={13} /> New report
          </Button>
        </div>
      </div>

      {error && (
        <div role="alert" className="border-b border-destructive/20 bg-destructive/10 px-4 py-2 text-sm text-destructive">
          {error}
        </div>
      )}

      {creating && (
        <div className="flex items-center gap-2 border-b border-border bg-panel-2/60 px-4 py-2">
          <Input
            autoFocus
            aria-label="new report title"
            className="h-8 w-72 text-xs"
            placeholder="New report title…"
            value={newTitle}
            onChange={(e) => setNewTitle(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && newTitle.trim()) void create();
              if (e.key === "Escape") setCreating(false);
            }}
          />
          <Button size="sm" disabled={!newTitle.trim()} onClick={() => void create()}>
            Create
          </Button>
          <Button size="sm" variant="ghost" onClick={() => setCreating(false)}>
            Cancel
          </Button>
        </div>
      )}

      {rows.length === 0 ? (
        <AppEmptyState
          icon={FileText}
          title={query ? "No reports match." : "No reports yet."}
          description={query ? "Clear the filter to see all reports." : "Create one to author a branded, panel-bearing report."}
        />
      ) : (
        <ul className="min-h-0 flex-1 overflow-auto p-2" data-testid="reports-roster">
          {rows.map((r) => (
            <li key={r.id} className="flex items-center gap-2 rounded-md px-2 py-1.5 hover:bg-panel-2/60">
              <Button
                variant="ghost"
                className="h-auto flex-1 justify-start truncate p-0 font-medium text-fg hover:bg-transparent hover:text-accent"
                onClick={() => onOpen(bare(r.id))}
              >
                <FileText size={13} className="mr-2 shrink-0 text-muted" />
                <span className="truncate">{r.title}</span>
              </Button>
              <span className="font-mono text-xs text-muted">{bare(r.id)}</span>
              <Button
                aria-label={`delete ${r.title}`}
                variant="ghost"
                size="icon"
                className="h-7 w-7 text-muted hover:text-destructive"
                onClick={() => setPendingDelete(r)}
              >
                <Trash2 size={13} />
              </Button>
            </li>
          ))}
        </ul>
      )}

      {pendingDelete && (
        <ConfirmDestructive
          title={`Delete ${pendingDelete.title}`}
          consequence="This report and its blocks will be removed (soft-delete)."
          reversible={false}
          escalation="none"
          confirmLabel="Delete"
          onConfirm={() => {
            void deleteReport(pendingDelete.id).then(refresh);
            setPendingDelete(null);
          }}
          onCancel={() => setPendingDelete(null)}
        />
      )}
    </>
  );
}

function bare(id: string): string {
  const i = id.indexOf(":");
  return i >= 0 ? id.slice(i + 1) : id;
}
