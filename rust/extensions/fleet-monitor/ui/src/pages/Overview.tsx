import { Radar } from "lucide-react";
import { Outlet } from "react-router-dom";

import { TabBar, type TabItem } from "@/components/ui";
import { useCtx } from "@/app/useCtx";

const TABS: TabItem[] = [
  { to: "/", label: "Nodes", end: true },
  { to: "/alerts", label: "Alerts" },
];

/** The PARENT page: header (Radar + title + workspace) and the sub-nav, with an Outlet where the
 *  nested child routes (Nodes / Alerts) render. */
export function Overview() {
  const { workspace } = useCtx();
  return (
    <div className="flex h-full w-full flex-col gap-4 bg-bg p-4 text-fg">
      <header className="flex items-center gap-2">
        <Radar className="h-5 w-5 text-accent" aria-hidden />
        <h1 className="text-base font-semibold tracking-tight">Fleet Monitor</h1>
        <span className="ml-2 rounded border border-border bg-panel px-2 py-0.5 text-xs text-muted">
          {workspace}
        </span>
      </header>
      <TabBar items={TABS} />
      <div className="min-h-0 flex-1">
        <Outlet />
      </div>
    </div>
  );
}
