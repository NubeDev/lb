// The Settings surface (user-prefs + agent-config scopes) — the long-deferred "settings surface" the
// prefs crate named, plus the workspace agent picker. A tabbed section: Preferences (per-user, always
// available — every member edits their OWN presentation axes) and Agent (the workspace's default
// runtime + model endpoint; editable by an admin, read-only for a member). Built on the shared
// shadcn primitives (Tabs + AppPageHeader) like AdminView. Markup + tab state only; each tab owns its
// own data + writes against the real gateway verbs.

import { useState } from "react";
import { Settings } from "lucide-react";

import { AppPageHeader } from "@/components/app/page-header";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { PreferencesTab } from "./PreferencesTab";
import { AgentTab } from "./AgentTab";

type Tab = "preferences" | "agent";

interface Props {
  ws: string;
  /** The session's caps — gate which controls are editable (display convenience; server is the wall). */
  caps: string[] | undefined;
}

export function SettingsView({ ws, caps }: Props) {
  const [tab, setTab] = useState<Tab>("preferences");

  return (
    <section className="flex h-full min-w-0 flex-col bg-bg text-fg">
      <AppPageHeader
        icon={Settings}
        title="Settings"
        description="Your preferences and the workspace agent."
        workspace={ws}
      />
      <Tabs value={tab} onValueChange={(v) => setTab(v as Tab)} className="min-h-0 flex-1">
        <TabsList className="m-2 flex-wrap">
          <TabsTrigger value="preferences" aria-label="Preferences">
            Preferences
          </TabsTrigger>
          <TabsTrigger value="agent" aria-label="Agent">
            Agent
          </TabsTrigger>
        </TabsList>
        <TabsContent value="preferences" className="min-h-0 flex-1 overflow-y-auto">
          <PreferencesTab ws={ws} caps={caps} />
        </TabsContent>
        <TabsContent value="agent" className="min-h-0 flex-1 overflow-y-auto">
          <AgentTab ws={ws} caps={caps} />
        </TabsContent>
      </Tabs>
    </section>
  );
}
