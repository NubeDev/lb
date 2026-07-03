// The Studio section shell: one page, two route-driven tabs — Extensions (manage installed) and
// Build (the scaffold→build→publish wizard). Tabs are URL-addressable (`/studio/extensions`,
// `/studio/build`) so each is deep-linkable and back/forward works; the active tab comes from the
// route, not local state (unlike the older local-state Tabs in AdminView). Each tab is a distinct
// CoreSurface with its OWN cap (`extensions`→ext.list, `build`→devkit.templates): a session sees a
// tab only when its cap is allowed, and the shell shows just the tabs it's permitted. The gateway
// re-checks every verb regardless — hiding a tab is display convenience, not the boundary. The route
// owns navigation (`onSelectTab`), keeping this feature decoupled from the router's route types.

import { Wrench } from "lucide-react";

import { AppPageHeader } from "@/components/app/page-header";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";

/** The tabs the Studio section can show. Each maps to a CoreSurface whose cap gates the tab. */
export type StudioTab = "extensions" | "build";

interface Props {
  ws: string;
  /** The active tab (from the route). */
  tab: StudioTab;
  /** Which tabs the session may SEE (caller cap-gates: extensions→ext.list, build→devkit.templates). */
  allowedTabs: StudioTab[];
  /** Navigate to a tab — the route folds this into a URL change so the tab is deep-linkable. */
  onSelectTab: (tab: StudioTab) => void;
  /** The active tab's body (Extensions console or the Build wizard). */
  children: React.ReactNode;
}

const TAB_LABELS: Record<StudioTab, string> = {
  extensions: "Extensions",
  build: "Build",
};

export function StudioShell({ ws, tab, allowedTabs, onSelectTab, children }: Props) {
  return (
    <div className="flex h-full min-h-0 flex-col bg-bg text-fg">
      <AppPageHeader
        icon={Wrench}
        title="Studio"
        description="Manage installed extensions, and scaffold, build, and publish your own."
        workspace={ws}
      />
      <Tabs
        value={tab}
        onValueChange={(v) => onSelectTab(v as StudioTab)}
        className="flex min-h-0 flex-1 flex-col"
      >
        <TabsList className="m-2 flex-wrap self-start">
          {allowedTabs.map((t) => (
            <TabsTrigger key={t} value={t} aria-label={TAB_LABELS[t]}>
              {TAB_LABELS[t]}
            </TabsTrigger>
          ))}
        </TabsList>
        <div className="min-h-0 flex-1 overflow-hidden">{children}</div>
      </Tabs>
    </div>
  );
}
