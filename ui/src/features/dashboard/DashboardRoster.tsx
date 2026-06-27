// The dashboard roster — the left list of dashboards the caller can reach + a create control
// (dashboard scope). Selecting one loads it into the grid; creating one UPSERTs an empty dashboard.
// The roster is exactly the set `dashboard.list` returns (own + team-shared + workspace) — the
// gateway membership-filters it, so a non-member never sees a dashboard's title here.

import { useState } from "react";
import { LayoutDashboard, Plus } from "lucide-react";

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
    <aside className="flex w-64 shrink-0 flex-col border-r border-border bg-panel shadow-sm shadow-black/5">
      <div className="flex items-center gap-2 border-b border-border px-3 py-3">
        <input
          aria-label="new dashboard title"
          placeholder="New dashboard…"
          className="control-field-sm min-w-0 flex-1"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && create()}
        />
        <button
          aria-label="create dashboard"
          className="soft-button-sm px-2"
          onClick={create}
        >
          <Plus size={14} />
        </button>
      </div>
      <ul className="flex-1 space-y-1 overflow-auto p-2">
        {roster.length === 0 && (
          <li className="rounded-md border border-dashed border-border bg-bg/60 px-3 py-3 text-xs text-muted">
            No dashboards yet.
          </li>
        )}
        {roster.map((d) => (
          <li key={d.id}>
            <button
              aria-label={`select dashboard ${d.id}`}
              className={`flex w-full items-center gap-2 rounded-md border px-2.5 py-2 text-left text-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/25 ${
                selectedId === d.id
                  ? "border-accent/25 bg-accent/15 text-accent shadow-sm shadow-black/5"
                  : "border-transparent text-fg hover:border-border hover:bg-bg"
              }`}
              onClick={() => onSelect(d.id)}
            >
              <LayoutDashboard size={14} className="shrink-0" />
              <span className="truncate">{d.title}</span>
              <span className="ml-auto text-[10px] uppercase text-muted">{d.visibility}</span>
            </button>
          </li>
        ))}
      </ul>
    </aside>
  );
}
