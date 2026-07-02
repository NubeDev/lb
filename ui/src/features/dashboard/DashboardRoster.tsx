// The dashboard roster — the left list of dashboards the caller can reach + a create control
// (dashboard scope). Selecting one loads it into the grid; creating one UPSERTs an empty dashboard.
// The roster is exactly the set `dashboard.list` returns (own + team-shared + workspace) — the
// gateway membership-filters it, so a non-member never sees a dashboard's title here. On the shared
// `AppRail` chrome + shadcn primitives (ui-standards-scope), matching Flows' rail.

import { useState } from "react";
import { LayoutDashboard, Plus } from "lucide-react";

import { AppRail } from "@/components/app/rail";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import type { DashboardSummary } from "@/lib/dashboard";

interface Props {
  roster: DashboardSummary[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onCreate: (id: string, title: string) => void;
}

/** Slugify a title into a stable, unique-ish id (the record id `dashboard:{id}`). */
function slug(title: string): string {
  return title
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

export function DashboardRoster({ roster, selectedId, onSelect, onCreate }: Props) {
  const [title, setTitle] = useState("");

  const create = () => {
    const t = title.trim();
    if (!t) return;
    const id = slug(t) || `dash-${roster.length + 1}`;
    onCreate(id, t);
    setTitle("");
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
          return (
            <li key={d.id}>
              <Button
                aria-label={`select dashboard ${d.id}`}
                variant="ghost"
                onClick={() => onSelect(d.id)}
                className={cn(
                  "h-auto w-full justify-start gap-2 border px-2.5 py-2 text-left text-sm font-normal",
                  active
                    ? "border-accent/25 bg-accent/15 text-accent shadow-sm shadow-black/5 hover:bg-accent/15"
                    : "border-transparent text-fg hover:border-border hover:bg-bg",
                )}
              >
                <LayoutDashboard size={14} className="shrink-0" />
                <span className="min-w-0 flex-1 truncate">{d.title}</span>
                <span className="text-[10px] uppercase text-muted">{d.visibility}</span>
              </Button>
            </li>
          );
        })}
      </ul>
    </AppRail>
  );
}
