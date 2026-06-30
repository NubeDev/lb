// The Access console shell (access-console scope) — an access-first rebuild of the admin console. A
// tabbed section whose LANDING is the Access overview (people/teams/roles/keys at a glance + the
// security-posture tiles), followed by the access-graph views: People (effective caps + provenance),
// Teams, Roles (now deletable), Workspaces, and API Keys. Each tab is cap-gated for DISPLAY ONLY —
// the gateway re-checks every verb server-side, so hiding a tab is convenience, never the boundary.
// Built on shadcn primitives (Tabs + AppPageHeader) per ui-standards; responsive (tabs wrap, the
// overview grid is 1→4 cols). Markup + tab state only.

import { useState } from "react";
import { Shield } from "lucide-react";

import { AppPageHeader } from "@/components/app/page-header";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { CAP, hasCap } from "@/lib/session";
import { AccessOverview } from "./access/AccessOverview";
import { ApiKeysAdmin } from "./ApiKeysAdmin";
import { PeopleAdmin } from "./PeopleAdmin";
import { TeamsAdmin } from "./TeamsAdmin";
import { RolesAdmin } from "./RolesAdmin";
import { WorkspacesAdmin } from "./WorkspacesAdmin";

type Tab = "overview" | "people" | "teams" | "roles" | "workspaces" | "apikeys";

interface Props {
  ws: string;
  /** The session's caps — gate which tabs to SHOW (display convenience; the server is the boundary). */
  caps: string[] | undefined;
}

export function AdminView({ ws, caps }: Props) {
  const tabs: { key: Tab; label: string; show: boolean }[] = [
    // The overview lands whenever the admin section is visible at all.
    { key: "overview", label: "Overview", show: hasCap(caps, CAP.userManage) || hasCap(caps, CAP.teamsManage) || hasCap(caps, CAP.grantsAssign) || hasCap(caps, CAP.workspaceDelete) || hasCap(caps, CAP.apikeyManage) },
    { key: "people", label: "People", show: hasCap(caps, CAP.userManage) },
    { key: "teams", label: "Teams", show: hasCap(caps, CAP.teamsManage) },
    { key: "roles", label: "Roles", show: hasCap(caps, CAP.grantsAssign) },
    { key: "workspaces", label: "Workspaces", show: hasCap(caps, CAP.workspaceDelete) },
    { key: "apikeys", label: "API Keys", show: hasCap(caps, CAP.apikeyManage) },
  ];
  const visible = tabs.filter((t) => t.show);
  const [tab, setTab] = useState<Tab>(visible[0]?.key ?? "overview");
  const active = visible.some((t) => t.key === tab) ? tab : visible[0]?.key ?? "overview";

  return (
    <section className="flex h-full min-w-0 flex-col bg-bg text-fg">
      <AppPageHeader
        icon={Shield}
        title="Access console"
        description="Access-first people, teams, roles, keys, and workspace controls."
        workspace={ws}
      />
      <Tabs value={active} onValueChange={(v) => setTab(v as Tab)} className="min-h-0 flex-1">
        <TabsList className="m-2 flex-wrap">
          {visible.map((t) => (
            <TabsTrigger key={t.key} value={t.key} aria-label={t.label}>
              {t.label}
            </TabsTrigger>
          ))}
        </TabsList>
        <TabsContent value="overview" className="min-h-0 flex-1">
          <AccessOverview ws={ws} caps={caps} onJump={(j) => setTab(j)} />
        </TabsContent>
        <TabsContent value="people" className="min-h-0 flex-1">
          <PeopleAdmin ws={ws} caps={caps} />
        </TabsContent>
        <TabsContent value="teams" className="min-h-0 flex-1">
          <TeamsAdmin ws={ws} />
        </TabsContent>
        <TabsContent value="roles" className="min-h-0 flex-1">
          <RolesAdmin ws={ws} caps={caps} />
        </TabsContent>
        <TabsContent value="workspaces" className="min-h-0 flex-1">
          <WorkspacesAdmin ws={ws} />
        </TabsContent>
        <TabsContent value="apikeys" className="min-h-0 flex-1">
          <ApiKeysAdmin ws={ws} />
        </TabsContent>
      </Tabs>
    </section>
  );
}
