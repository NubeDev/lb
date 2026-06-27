// The admin console shell (admin-console redesign) — a tabbed section over FOUR relationship-first
// surfaces: People (users + their teams/roles/access), Teams (records + inline members + access),
// Roles (the real cap-bundle editor), and Workspaces (lifecycle). The old separate Users / Members /
// Grants tabs are folded in (members live under Teams; grant/role assignment lives in each subject's
// detail). Each tab is cap-gated for DISPLAY ONLY — the gateway re-checks every verb server-side, so
// hiding a tab is convenience, never the security boundary. Markup + tab state only.

import { useState } from "react";

import { CAP, hasCap } from "@/lib/session";
import { PeopleAdmin } from "./PeopleAdmin";
import { TeamsAdmin } from "./TeamsAdmin";
import { RolesAdmin } from "./RolesAdmin";
import { WorkspacesAdmin } from "./WorkspacesAdmin";

type Tab = "people" | "teams" | "roles" | "workspaces";

interface Props {
  ws: string;
  /** The session's caps — gate which tabs to SHOW (display convenience; the server is the boundary). */
  caps: string[] | undefined;
}

export function AdminView({ ws, caps }: Props) {
  const tabs: { key: Tab; label: string; show: boolean }[] = [
    { key: "people", label: "People", show: hasCap(caps, CAP.userManage) },
    { key: "teams", label: "Teams", show: hasCap(caps, CAP.teamsManage) },
    { key: "roles", label: "Roles", show: hasCap(caps, CAP.grantsAssign) },
    { key: "workspaces", label: "Workspaces", show: hasCap(caps, CAP.workspaceDelete) },
  ];
  const visible = tabs.filter((t) => t.show);
  const [tab, setTab] = useState<Tab>(visible[0]?.key ?? "people");
  const active = visible.some((t) => t.key === tab) ? tab : visible[0]?.key;

  return (
    <div className="flex h-full flex-col">
      <nav className="flex gap-1 border-b border-border bg-panel px-2 py-1" role="tablist">
        {visible.map((t) => (
          <button
            key={t.key}
            role="tab"
            aria-selected={active === t.key}
            aria-label={t.label}
            className={`rounded px-3 py-1 text-xs ${
              active === t.key ? "bg-accent/15 text-accent" : "text-muted hover:bg-bg"
            }`}
            onClick={() => setTab(t.key)}
          >
            {t.label}
          </button>
        ))}
      </nav>
      <div className="min-h-0 flex-1">
        {active === "people" && <PeopleAdmin ws={ws} />}
        {active === "teams" && <TeamsAdmin ws={ws} />}
        {active === "roles" && <RolesAdmin ws={ws} caps={caps} />}
        {active === "workspaces" && <WorkspacesAdmin ws={ws} />}
      </div>
    </div>
  );
}
