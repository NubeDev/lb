// The Settings surface (user-prefs + agent-config + theme + branding) — the long-deferred "settings
// surface" the prefs crate named, plus the workspace agent picker, the theme customizer, and the
// admin-owned workspace brand. A tabbed section: Preferences (per-user presentation axes), Branding
// (admin workspace identity), Theme (the full customizer — presets/radius/mode/import/brand-colors +
// sidebar layout), and Agent (the workspace default runtime/endpoint). Tabs are URL-routable
// (`/settings/<tab>`): the active tab and switching are driven by the router, so each tab is
// deep-linkable and the browser back button works. Markup + tab wiring only; each tab owns its own
// data + writes against the real gateway verbs.

import { Settings } from "lucide-react";

import { AppPageHeader } from "@/components/app/page-header";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { PreferencesTab } from "./PreferencesTab";
import { AgentTab } from "./AgentTab";
import { BrandingTab } from "./BrandingTab";
import { SidebarTab } from "./SidebarTab";
import { ThemeSettingsTab } from "./ThemeSettingsTab";

const TABS = ["preferences", "branding", "theme", "sidebar", "agent"] as const;
export type SettingsTab = (typeof TABS)[number];

/** Coerce an arbitrary URL segment to a valid tab (unknown → the default). */
export function coerceSettingsTab(value: string | undefined): SettingsTab {
  return TABS.includes(value as SettingsTab) ? (value as SettingsTab) : "preferences";
}

interface Props {
  ws: string;
  /** The session's caps — gate which controls are editable (display convenience; server is the wall). */
  caps: string[] | undefined;
  /** The active tab, from the URL (`/settings/<tab>`). */
  tab: string;
  /** Navigate to another tab (updates the URL). */
  onTabChange: (tab: SettingsTab) => void;
}

export function SettingsView({ ws, caps, tab, onTabChange }: Props) {
  const active = coerceSettingsTab(tab);

  return (
    <section className="flex h-full min-w-0 flex-col bg-bg text-fg">
      <AppPageHeader
        icon={Settings}
        title="Settings"
        description="Your preferences, the workspace brand, your theme, and the workspace agent."
        workspace={ws}
      />
      <Tabs value={active} onValueChange={(v) => onTabChange(coerceSettingsTab(v))} className="min-h-0 flex-1">
        {/* The tab row sits in the SAME centered column as the tab content, sized to its pills — a
            full-width boxed strip over a centered form read as two competing layouts. */}
        <TabsList className="mx-auto mt-3 w-fit flex-wrap self-center">
          <TabsTrigger value="preferences" aria-label="Preferences">
            Preferences
          </TabsTrigger>
          <TabsTrigger value="branding" aria-label="Branding">
            Branding
          </TabsTrigger>
          <TabsTrigger value="theme" aria-label="Theme">
            Theme
          </TabsTrigger>
          <TabsTrigger value="sidebar" aria-label="Sidebar">
            Sidebar
          </TabsTrigger>
          <TabsTrigger value="agent" aria-label="Agent">
            Agent
          </TabsTrigger>
        </TabsList>
        <TabsContent value="preferences" className="min-h-0 flex-1 overflow-y-auto">
          <PreferencesTab ws={ws} caps={caps} />
        </TabsContent>
        <TabsContent value="branding" className="min-h-0 flex-1 overflow-y-auto">
          <BrandingTab ws={ws} caps={caps} />
        </TabsContent>
        <TabsContent value="theme" className="min-h-0 flex-1 overflow-y-auto">
          <ThemeSettingsTab />
        </TabsContent>
        <TabsContent value="sidebar" className="min-h-0 flex-1 overflow-y-auto">
          {/* hide-and-pins scope: the workspace sidebar hidden-set (admin curation; member read-only). */}
          <SidebarTab ws={ws} caps={caps} />
        </TabsContent>
        <TabsContent value="agent" className="min-h-0 flex-1 overflow-y-auto">
          <AgentTab ws={ws} caps={caps} />
        </TabsContent>
      </Tabs>
    </section>
  );
}
