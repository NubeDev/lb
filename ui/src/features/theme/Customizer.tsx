// The Customizer — a slide-out Sheet with the Theme and Layout tabs, reachable from the nav footer.
// Theme = presets/radius/mode/import/brand-colors/reset/admin-default; Layout = sidebar variant/
// collapsible/side. Both write the same theme layer (one `ui_theme` prefs blob). The compact
// `ThemeSwitcher` stays as the quick mode/preset toggle in the collapsed rail. One component per file.

import * as React from "react";

import { LayoutTemplate, Palette } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

import { LayoutTab } from "./LayoutTab";
import { ThemeTab } from "./ThemeTab";

export function Customizer() {
  const [open, setOpen] = React.useState(false);
  const [tab, setTab] = React.useState("theme");

  return (
    <>
      <Button
        type="button"
        variant="ghost"
        size="sm"
        aria-label="Customize theme"
        className="w-full justify-start gap-2 text-muted hover:text-fg group-data-[collapsible=icon]:w-8 group-data-[collapsible=icon]:justify-center group-data-[collapsible=icon]:px-0"
        onClick={() => setOpen(true)}
      >
        <Palette className="h-4 w-4" />
        <span className="group-data-[collapsible=icon]:hidden">Customize</span>
      </Button>

      <Sheet open={open} onOpenChange={setOpen}>
        <SheetContent side="right" className="w-full overflow-y-auto p-0 sm:max-w-sm">
          <SheetHeader className="border-b border-border">
            <SheetTitle>Customizer</SheetTitle>
            <SheetDescription>Theme and sidebar layout — applied live.</SheetDescription>
          </SheetHeader>
          <Tabs value={tab} onValueChange={setTab}>
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
        </SheetContent>
      </Sheet>
    </>
  );
}
