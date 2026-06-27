// The admin console shell (admin-console scope): a tabbed section over the five entity sub-views
// (workspaces · users · teams · members · roles/grants). Each tab is cap-gated for DISPLAY ONLY — the
// gateway re-checks every verb server-side, so hiding a tab is convenience, never the security
// boundary (the forged-call deny is proven in Rust: role/gateway/tests/admin_routes_test.rs). Markup
// + tab state only; each sub-view owns its data.

import { useState } from "react";

import { CAP, hasCap } from "@/lib/session";
import { WorkspacesAdmin } from "./WorkspacesAdmin";
import { UsersAdmin } from "./UsersAdmin";
import { TeamsAdmin } from "./TeamsAdmin";
import { MembersAdmin } from "./MembersAdmin";
import { GrantsAdmin } from "./GrantsAdmin";

type Tab = "workspaces" | "users" | "teams" | "members" | "grants";

interface Props {
  ws: string;
  /** The session's caps — gate which tabs to SHOW (display convenience; the server is the boundary). */
  caps: string[] | undefined;
}

export function AdminView({ ws, caps }: Props) {
  // Each tab shows only if the session carries the controlling cap (per-control gating, the scope's
  // lean). The server re-checks regardless.
  const tabs: { key: Tab; label: string; show: boolean }[] = [
    { key: "workspaces", label: "Workspaces", show: hasCap(caps, CAP.workspaceDelete) },
    { key: "users", label: "Users", show: hasCap(caps, CAP.userManage) },
    { key: "teams", label: "Teams", show: hasCap(caps, CAP.teamsManage) },
    { key: "members", label: "Members", show: hasCap(caps, CAP.teamsManage) },
    { key: "grants", label: "Roles & grants", show: hasCap(caps, CAP.grantsAssign) },
  ];
  const visible = tabs.filter((t) => t.show);
  const [tab, setTab] = useState<Tab>(visible[0]?.key ?? "users");
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
        {active === "workspaces" && <WorkspacesAdmin ws={ws} />}
        {active === "users" && <UsersAdmin ws={ws} />}
        {active === "teams" && <TeamsAdmin ws={ws} />}
        {active === "members" && <MembersAdmin ws={ws} />}
        {active === "grants" && <GrantsAdmin ws={ws} />}
      </div>
    </div>
  );
}
