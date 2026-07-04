// The Settings → Theme tab — the full theme customizer, moved out of the old nav-footer sheet into
// Settings. Hosts the Theme controls (presets/radius/mode/import/brand-colors + admin workspace
// default) and the Layout controls (sidebar variant/collapsible/side) as two sub-tabs, reusing the
// `features/theme` components. Applied live; persisted per-member via the theme layer's prefs sync.
// One component per file (FILE-LAYOUT).

import * as React from "react";

import { LayoutTemplate, Palette } from "lucide-react";

import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { LayoutTab, ThemeTab } from "@/features/theme";

export function ThemeSettingsTab() {
  const [sub, setSub] = React.useState("theme");
  return (
    <div className="mx-auto max-w-xl">
      <Tabs value={sub} onValueChange={setSub}>
        <TabsList className="mx-4 mt-4 grid grid-cols-2">
          <TabsTrigger value="theme">
            <Palette className="mr-1.5 h-4 w-4" />
            Theme
          </TabsTrigger>
          <TabsTrigger value="layout">
            <LayoutTemplate className="mr-1.5 h-4 w-4" />
            Layout
          </TabsTrigger>
        </TabsList>
        <TabsContent value="theme">
          <ThemeTab />
        </TabsContent>
        <TabsContent value="layout">
          <LayoutTab />
        </TabsContent>
      </Tabs>
    </div>
  );
}
