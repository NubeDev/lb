// The dashboard roster — the left list of dashboards the caller can reach + a create control
// (dashboard scope). Selecting one loads it into the grid; creating one UPSERTs an empty dashboard.
// The roster is an AUTHORING surface: it renders only for an ADMIN (`DashboardView` mounts it behind
// `canEdit = isAdmin(caps)`, viewer-mode scope). `canEdit` also gates per-item **rename** (inline
// title edit → title-only `dashboard.save`) and **delete** (routed through the shared
// `ConfirmDestructive` gate; the host re-checks owner + cap). The roster is exactly the set
// `dashboard.list` returns (own + team-shared + workspace) — the gateway membership-filters it, so a
// non-member never sees a dashboard's title here. Chrome/behavior live in the shared `RosterRail`
// (components/app/roster.tsx); this file only maps dashboards onto it: the slug for new ids, the
// visibility badge, and the delete confirm (the rail hands back the item, the feature owns the gate).

import { useState } from "react";
import { LayoutDashboard } from "lucide-react";

import { RosterRail } from "@/components/app/roster";
import { ConfirmDestructive } from "@/features/confirm/ConfirmDestructive";
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
  // The dashboard pending a delete confirm — the rail hands the item back; the feature owns the gate.
  const [pendingDelete, setPendingDelete] = useState<DashboardSummary | null>(null);

  return (
    <>
      <RosterRail
        noun="dashboard"
        icon={LayoutDashboard}
        items={roster.map((d) => ({
          ...d,
          badge: d.visibility !== "workspace" ? d.visibility : undefined,
          // Page-settings icon/colour ride the summary, so the switcher paints them per row (falling
          // back to the rail's shared LayoutDashboard when unset).
          icon: d.icon || undefined,
          iconColor: d.color || undefined,
        }))}
        selectedId={selectedId}
        onSelect={onSelect}
        emptyText="No dashboards yet."
        createPlaceholder="New dashboard…"
        onCreate={(title) => onCreate(slug(title) || `dash-${roster.length + 1}`, title)}
        onCollapse={onCollapse}
        onRename={canEdit ? onRename : undefined}
        onRemove={canEdit ? (item) => setPendingDelete(item) : undefined}
      />

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
    </>
  );
}
