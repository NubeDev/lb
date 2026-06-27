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
    <aside className="flex w-56 flex-col border-r border-border bg-panel">
      <div className="flex items-center gap-2 border-b border-border px-3 py-2">
        <input
          aria-label="new dashboard title"
          placeholder="New dashboard…"
          className="min-w-0 flex-1 rounded border border-border bg-bg px-2 py-1 text-xs"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && create()}
        />
        <button
          aria-label="create dashboard"
          className="rounded bg-accent/15 p-1 text-accent"
          onClick={create}
        >
          <Plus size={14} />
        </button>
      </div>
      <ul className="flex-1 overflow-auto py-1">
        {roster.length === 0 && (
          <li className="px-3 py-2 text-xs text-muted">No dashboards yet.</li>
        )}
        {roster.map((d) => (
          <li key={d.id}>
            <button
              aria-label={`select dashboard ${d.id}`}
              className={`flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm ${
                selectedId === d.id ? "bg-accent/15 text-accent" : "text-fg hover:bg-bg"
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
