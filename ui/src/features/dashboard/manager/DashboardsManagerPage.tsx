// The Dashboards manager (dashboard scope, dashboard-management UX) — a full-CRUD table over every
// dashboard the caller can reach, plus bundle import/export. This is the "manage all dashboards" page a
// nav entry or the dashboard toolbar links to; the live grid (`DashboardView`) stays the VIEW/AUTHOR
// surface, this is the LIBRARY surface. Every mutation funnels through the shipped `dashboard.*` verbs
// (create/rename/delete via `useDashboard`; duplicate = get + save; export/import via `useDashboardIo`),
// each capability-gated + workspace-walled server-side (§5/§6). Admin-only in the UI (`isAdmin`) — it is
// an authoring surface; the host re-checks `dashboard.save`/`.delete` regardless. One responsibility:
// the management table + its toolbar (dialogs + row actions live in their own files).

import { useMemo, useState } from "react";
import {
  Copy,
  Download,
  LayoutDashboard,
  Pencil,
  Plus,
  Trash2,
  Upload,
} from "lucide-react";

import { AppPage } from "@/components/app/page";
import { AppEmptyState } from "@/components/app/empty-state";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { ConfirmDestructive } from "@/features/confirm/ConfirmDestructive";
import {
  getDashboard,
  saveDashboard,
  dashboardToPortable,
  slugFromTitle,
} from "@/lib/dashboard";
import type { DashboardSummary, PortableDashboard } from "@/lib/dashboard";
import { isAdmin } from "@/lib/session";
import { useAppRoutingContext } from "@/features/routing/RoutingContextProvider";
import { DashboardCacheProvider } from "../cache/DashboardQueryProvider";
import { useDashboard } from "../useDashboard";
import { useDashboardIo } from "../io/useDashboardIo";
import { ImportDialog } from "../io/ImportDialog";

interface Props {
  ws: string;
  /** Open a dashboard in the live grid (`/t/$ws/dashboards?d=<id>`). */
  onOpen: (id: string) => void;
}

/** Wrapped in the per-visit read cache like `DashboardView` — keyed on `ws` so a workspace switch
 *  remounts with fresh ws-scoped keys (no cross-ws bleed). */
export function DashboardsManagerPage(props: Props) {
  return (
    <DashboardCacheProvider key={props.ws} ws={props.ws}>
      <ManagerInner {...props} />
    </DashboardCacheProvider>
  );
}

function ManagerInner({ ws, onOpen }: Props) {
  const dash = useDashboard(ws);
  const io = useDashboardIo();
  const { caps } = useAppRoutingContext();
  const canEdit = isAdmin(caps);

  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [query, setQuery] = useState("");
  const [creating, setCreating] = useState(false);
  const [newTitle, setNewTitle] = useState("");
  const [renaming, setRenaming] = useState<DashboardSummary | null>(null);
  const [renameTitle, setRenameTitle] = useState("");
  const [pendingDelete, setPendingDelete] = useState<DashboardSummary | null>(
    null,
  );
  const [importOpen, setImportOpen] = useState(false);

  const rows = useMemo(() => {
    const q = query.trim().toLowerCase();
    const list = q
      ? dash.roster.filter(
          (d) => d.title.toLowerCase().includes(q) || bare(d.id).includes(q),
        )
      : dash.roster;
    return [...list].sort((a, b) => b.updated_ts - a.updated_ts);
  }, [dash.roster, query]);

  const allChecked = rows.length > 0 && rows.every((d) => selected.has(d.id));
  const toggleAll = () =>
    setSelected(allChecked ? new Set() : new Set(rows.map((d) => d.id)));
  const toggle = (id: string) =>
    setSelected((prev) => {
      const next = new Set(prev);
      next.has(id) ? next.delete(id) : next.add(id);
      return next;
    });

  const exportSelected = () => {
    const ids = rows.filter((d) => selected.has(d.id)).map((d) => d.id);
    if (ids.length === 0) return;
    void io.exportBundle(ids, [], undefined, new Date().toISOString());
  };
  const exportOne = (id: string) =>
    void io.exportBundle([id], [], undefined, new Date().toISOString());

  const duplicate = async (id: string) => {
    // Duplicate = read the full record (the roster carries only summaries) + save a fresh, non-colliding
    // copy carrying the cloned cells/variables. One `dashboard.save` — `useDashboard.create` only takes
    // id+title, so we call the api verb directly to include the layout.
    const full = await getDashboard(id);
    const p: PortableDashboard = dashboardToPortable(full);
    const taken = new Set(dash.roster.map((d) => bare(d.id)));
    const base = `${slugFromTitle(p.title)}-copy`;
    let fresh = base;
    let n = 2;
    while (taken.has(fresh)) fresh = `${base}-${n++}`;
    await saveDashboard(fresh, `${p.title} (copy)`, p.cells, p.variables ?? []);
    await dash.refresh();
  };

  const doImport = async (
    bundle: Parameters<typeof io.importBundle>[0],
    mode: Parameters<typeof io.importBundle>[1],
  ) => {
    const outcome = await io.importBundle(bundle, mode);
    await dash.refresh();
    return outcome;
  };

  const selectedCount = rows.filter((d) => selected.has(d.id)).length;

  return (
    <AppPage
      label="dashboards manager"
      icon={LayoutDashboard}
      title="Dashboards"
      description="Manage, import, and export every dashboard in this workspace."
      workspace={ws}
      error={dash.error ?? io.error}
      actions={
        <>
          <Button
            variant="outline"
            size="sm"
            onClick={() => setImportOpen(true)}
          >
            <Upload size={13} /> Import
          </Button>
          <Button
            variant="outline"
            size="sm"
            disabled={selectedCount === 0 || io.busy}
            onClick={exportSelected}
            title={
              selectedCount === 0 ? "Select dashboards to export" : undefined
            }
          >
            <Download size={13} /> Export
            {selectedCount ? ` (${selectedCount})` : ""}
          </Button>
          {canEdit && (
            <Button size="sm" onClick={() => setCreating(true)}>
              <Plus size={13} /> New dashboard
            </Button>
          )}
        </>
      }
    >
      <div className="flex min-w-0 flex-1 flex-col">
        <div className="flex items-center gap-2 border-b border-border px-4 py-2">
          <Input
            aria-label="filter dashboards"
            className="h-8 w-64 text-xs"
            placeholder="Filter by title or id…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          <span className="text-xs text-muted">
            {rows.length} dashboard{rows.length === 1 ? "" : "s"}
          </span>
        </div>

        {creating && (
          <div className="flex items-center gap-2 border-b border-border bg-panel-2/60 px-4 py-2">
            <Input
              autoFocus
              aria-label="new dashboard title"
              className="h-8 w-72 text-xs"
              placeholder="New dashboard title…"
              value={newTitle}
              onChange={(e) => setNewTitle(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && newTitle.trim()) submitCreate();
                if (e.key === "Escape") setCreating(false);
              }}
            />
            <Button
              size="sm"
              disabled={!newTitle.trim()}
              onClick={submitCreate}
            >
              Create
            </Button>
            <Button
              size="sm"
              variant="ghost"
              onClick={() => setCreating(false)}
            >
              Cancel
            </Button>
          </div>
        )}

        {rows.length === 0 ? (
          <AppEmptyState
            icon={LayoutDashboard}
            title={query ? "No dashboards match." : "No dashboards yet."}
            description={
              query
                ? "Clear the filter to see all dashboards."
                : "Create one, or import a bundle exported from another workspace or node."
            }
          />
        ) : (
          <div className="min-h-0 flex-1 overflow-auto">
            <Table>
              <TableHeader sticky>
                <TableRow>
                  <TableHead className="w-8">
                    <Checkbox
                      aria-label="select all dashboards"
                      checked={allChecked}
                      onChange={toggleAll}
                    />
                  </TableHead>
                  <TableHead>Title</TableHead>
                  <TableHead>Id</TableHead>
                  <TableHead>Visibility</TableHead>
                  <TableHead>Updated</TableHead>
                  <TableHead className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {rows.map((d) => (
                  <TableRow
                    key={d.id}
                    data-state={selected.has(d.id) ? "selected" : undefined}
                  >
                    <TableCell>
                      <Checkbox
                        aria-label={`select ${d.title}`}
                        checked={selected.has(d.id)}
                        onChange={() => toggle(d.id)}
                      />
                    </TableCell>
                    <TableCell>
                      <Button
                        variant="ghost"
                        className="h-auto max-w-[28ch] justify-start truncate p-0 font-medium text-fg hover:bg-transparent hover:text-accent hover:underline"
                        onClick={() => onOpen(bare(d.id))}
                      >
                        {d.title}
                      </Button>
                    </TableCell>
                    <TableCell className="font-mono text-muted">
                      {bare(d.id)}
                    </TableCell>
                    <TableCell>
                      <Badge
                        variant="outline"
                        className="rounded-full text-[10px]"
                      >
                        {d.visibility}
                      </Badge>
                    </TableCell>
                    <TableCell className="text-muted">
                      {fmtTime(d.updated_ts)}
                    </TableCell>
                    <TableCell>
                      <div className="flex items-center justify-end gap-0.5">
                        <IconBtn
                          label={`export ${d.title}`}
                          title="Export"
                          onClick={() => exportOne(d.id)}
                        >
                          <Download size={13} />
                        </IconBtn>
                        {canEdit && (
                          <IconBtn
                            label={`duplicate ${d.title}`}
                            title="Duplicate"
                            onClick={() => void duplicate(d.id)}
                          >
                            <Copy size={13} />
                          </IconBtn>
                        )}
                        {canEdit && (
                          <IconBtn
                            label={`rename ${d.title}`}
                            title="Rename"
                            onClick={() => {
                              setRenaming(d);
                              setRenameTitle(d.title);
                            }}
                          >
                            <Pencil size={13} />
                          </IconBtn>
                        )}
                        {canEdit && (
                          <IconBtn
                            label={`delete ${d.title}`}
                            title="Delete"
                            destructive
                            onClick={() => setPendingDelete(d)}
                          >
                            <Trash2 size={13} />
                          </IconBtn>
                        )}
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        )}
      </div>

      {renaming && (
        <RenameDialog
          title={renameTitle}
          onTitle={setRenameTitle}
          onCancel={() => setRenaming(null)}
          onSave={() => {
            void dash.rename(renaming.id, renameTitle);
            setRenaming(null);
          }}
        />
      )}

      {pendingDelete && (
        <ConfirmDestructive
          title={`Delete ${pendingDelete.title}`}
          consequence="This dashboard and its widget layout will be removed. It can be recreated but its current layout is not recoverable."
          reversible={false}
          escalation="none"
          confirmLabel="Delete"
          onConfirm={() => {
            void dash.remove(pendingDelete.id);
            setSelected((prev) => {
              const next = new Set(prev);
              next.delete(pendingDelete.id);
              return next;
            });
            setPendingDelete(null);
          }}
          onCancel={() => setPendingDelete(null)}
        />
      )}

      <ImportDialog
        open={importOpen}
        onOpenChange={setImportOpen}
        busy={io.busy}
        onImport={doImport}
      />
    </AppPage>
  );

  function submitCreate() {
    const t = newTitle.trim();
    if (!t) return;
    const taken = new Set(dash.roster.map((d) => bare(d.id)));
    let id = slugFromTitle(t);
    let n = 2;
    while (taken.has(id)) id = `${slugFromTitle(t)}-${n++}`;
    void dash.create(id, t);
    setNewTitle("");
    setCreating(false);
  }
}

function IconBtn({
  label,
  title,
  destructive,
  onClick,
  children,
}: {
  label: string;
  title: string;
  destructive?: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <Button
      aria-label={label}
      title={title}
      variant="ghost"
      size="icon"
      className={
        destructive
          ? "h-7 w-7 text-muted hover:bg-destructive/10 hover:text-destructive"
          : "h-7 w-7 text-muted hover:text-fg"
      }
      onClick={onClick}
    >
      {children}
    </Button>
  );
}

function RenameDialog({
  title,
  onTitle,
  onCancel,
  onSave,
}: {
  title: string;
  onTitle: (t: string) => void;
  onCancel: () => void;
  onSave: () => void;
}) {
  // A minimal inline modal reusing the shell overlay tokens (matches ConfirmDestructive's weight).
  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-overlay/60 p-4"
      role="dialog"
      aria-label="rename dashboard"
    >
      <div className="w-full max-w-sm rounded-lg border border-border bg-panel p-4 shadow-lg">
        <h2 className="mb-3 text-sm font-semibold">Rename dashboard</h2>
        <Input
          autoFocus
          aria-label="dashboard title"
          value={title}
          onChange={(e) => onTitle(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && title.trim()) onSave();
            if (e.key === "Escape") onCancel();
          }}
        />
        <div className="mt-4 flex justify-end gap-2">
          <Button variant="ghost" size="sm" onClick={onCancel}>
            Cancel
          </Button>
          <Button size="sm" disabled={!title.trim()} onClick={onSave}>
            Save
          </Button>
        </div>
      </div>
    </div>
  );
}

function bare(id: string): string {
  const i = id.indexOf(":");
  return i >= 0 ? id.slice(i + 1) : id;
}

function fmtTime(ts: number): string {
  if (!ts) return "—";
  try {
    return new Date(ts).toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return "—";
  }
}
