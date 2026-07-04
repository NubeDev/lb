// The dashboard roster — the left list of dashboards the caller can reach + a create control
// (dashboard scope). Selecting one loads it into the grid; creating one UPSERTs an empty dashboard.
// The roster is an AUTHORING surface: it renders only for an ADMIN (`DashboardView` mounts it behind
// `canEdit = isAdmin(caps)`, viewer-mode scope). `canEdit` also gates per-item **rename** (inline
// title edit → title-only `dashboard.save`) and **delete** (routed through the shared
// `ConfirmDestructive` gate; the host re-checks owner + cap). The roster is exactly the set
// `dashboard.list` returns
// (own + team-shared + workspace) — the gateway membership-filters it, so a non-member never sees a
// dashboard's title here. A single minimize affordance in the header (`onCollapse`) folds the rail to
// a thin strip (the symmetric expand lives in `DashboardView`). On the shared `AppRail` chrome +
// shadcn primitives (ui-standards-scope).

import { useState } from "react";
import { LayoutDashboard, PanelLeftClose, Pencil, Plus, Trash2, Check, X } from "lucide-react";

import { AppRail } from "@/components/app/rail";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ConfirmDestructive } from "@/features/confirm/ConfirmDestructive";
import { cn } from "@/lib/utils";
import type { DashboardSummary } from "@/lib/dashboard";

interface Props {
  roster: DashboardSummary[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onCreate: (id: string, title: string) => void;
  /** Rename a dashboard (title-only save). Only wired when the caller may edit. */
  onRename?: (id: string, title: string) => void;
  /** Delete a dashboard (owner-only, host-checked). Only wired when the caller may edit. */
  onRemove?: (id: string) => void;
  /** Whether the caller may author (the workspace-admin role, `isAdmin`) — gates rename/delete. The
   *  roster itself only mounts for an admin, so in practice this is always true when rendered. */
  canEdit?: boolean;
  /** Collapse the roster rail (a single minimize affordance in the header). Only wired when the caller
   *  may collapse — the host (DashboardView) supplies a symmetric expand control when collapsed. */
  onCollapse?: () => void;
}

/** Slugify a title into a stable, unique-ish id (the record id `dashboard:{id}`). */
function slug(title: string): string {
  return title
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

export function DashboardRoster({
  roster,
  selectedId,
  onSelect,
  onCreate,
  onRename,
  onRemove,
  canEdit = false,
  onCollapse,
}: Props) {
  const [title, setTitle] = useState("");
  // The dashboard currently being renamed inline, and the draft title.
  const [editingId, setEditingId] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  // The dashboard pending a delete confirm.
  const [pendingDelete, setPendingDelete] = useState<DashboardSummary | null>(null);

  const create = () => {
    const t = title.trim();
    if (!t) return;
    const id = slug(t) || `dash-${roster.length + 1}`;
    onCreate(id, t);
    setTitle("");
  };

  const startEdit = (d: DashboardSummary) => {
    setEditingId(d.id);
    setDraft(d.title);
  };

  const commitEdit = () => {
    if (editingId && draft.trim()) onRename?.(editingId, draft);
    setEditingId(null);
    setDraft("");
  };

  const cancelEdit = () => {
    setEditingId(null);
    setDraft("");
  };

  return (
    <AppRail
      label="dashboard rail"
      header={
        <>
          <Input
            aria-label="new dashboard title"
            placeholder="New dashboard…"
            className="h-8 min-w-0 flex-1 text-xs"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && create()}
          />
          <Button
            aria-label="create dashboard"
            variant="outline"
            size="sm"
            className="h-8 px-2"
            onClick={create}
          >
            <Plus size={14} />
          </Button>
          {onCollapse && (
            <Button
              aria-label="minimize dashboard rail"
              variant="ghost"
              size="icon"
              className="h-8 w-8 shrink-0"
              title="Minimize"
              onClick={onCollapse}
            >
              <PanelLeftClose size={14} />
            </Button>
          )}
        </>
      }
    >
      <ul className="space-y-1">
        {roster.length === 0 && (
          <li className="rounded-md border border-dashed border-border bg-bg/60 px-3 py-3 text-xs text-muted">
            No dashboards yet.
          </li>
        )}
        {roster.map((d) => {
          const active = selectedId === d.id;
          if (editingId === d.id) {
            return (
              <li key={d.id} className="flex items-center gap-1">
                <Input
                  aria-label={`rename dashboard ${d.id}`}
                  className="h-8 min-w-0 flex-1 text-sm"
                  autoFocus
                  value={draft}
                  onChange={(e) => setDraft(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") commitEdit();
                    if (e.key === "Escape") cancelEdit();
                  }}
                />
                <Button
                  aria-label={`confirm rename ${d.id}`}
                  variant="ghost"
                  size="icon"
                  className="h-8 w-8 shrink-0"
                  onClick={commitEdit}
                >
                  <Check size={14} />
                </Button>
                <Button
                  aria-label={`cancel rename ${d.id}`}
                  variant="ghost"
                  size="icon"
                  className="h-8 w-8 shrink-0"
                  onClick={cancelEdit}
                >
                  <X size={14} />
                </Button>
              </li>
            );
          }
          return (
            <li key={d.id} className="group flex items-center gap-1">
              <Button
                aria-label={`select dashboard ${d.id}`}
                variant="ghost"
                onClick={() => onSelect(d.id)}
                className={cn(
                  "h-auto min-w-0 flex-1 justify-start gap-2 px-2.5 py-1.5 text-left text-[13px] font-normal",
                  active
                    ? "bg-accent/10 text-accent hover:bg-accent/10"
                    : "text-fg/90 hover:bg-fg/[0.06] hover:text-fg",
                )}
              >
                <LayoutDashboard size={14} className={cn("shrink-0", active ? "" : "text-muted")} />
                <span className="min-w-0 flex-1 truncate">{d.title}</span>
                {d.visibility !== "workspace" && (
                  <span className="shrink-0 text-[10px] font-medium text-muted/80">{d.visibility}</span>
                )}
              </Button>
              {canEdit && (
                <div className="flex shrink-0 items-center opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100">
                  <Button
                    aria-label={`rename dashboard ${d.id}`}
                    variant="ghost"
                    size="icon"
                    className="h-8 w-8"
                    title="Rename"
                    onClick={() => startEdit(d)}
                  >
                    <Pencil size={13} />
                  </Button>
                  <Button
                    aria-label={`delete dashboard ${d.id}`}
                    variant="ghost"
                    size="icon"
                    className="h-8 w-8 text-muted hover:text-danger"
                    title="Delete"
                    onClick={() => setPendingDelete(d)}
                  >
                    <Trash2 size={13} />
                  </Button>
                </div>
              )}
            </li>
          );
        })}
      </ul>

      {pendingDelete && (
        <ConfirmDestructive
          title={`Delete ${pendingDelete.title}`}
          consequence="This dashboard and its widget layout will be removed. It can be recreated but its current layout is not recoverable."
          reversible={false}
          escalation="none"
          confirmLabel="Delete"
          onConfirm={() => {
            onRemove?.(pendingDelete.id);
            setPendingDelete(null);
          }}
          onCancel={() => setPendingDelete(null)}
        />
      )}
    </AppRail>
  );
}
