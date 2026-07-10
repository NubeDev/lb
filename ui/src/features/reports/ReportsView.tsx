// ReportsView — the reports surface router entry (reports scope). Wraps the surface in AppPage chrome
// and switches between the roster (ReportsPage) and the editor (ReportEditor) on an internal `open`
// id state — a self-contained master/detail so the shell only mounts one route. Track E imports this
// into the router (`coreRoute("/reports", "reports", () => <ReportsView ws=… />)`). One responsibility:
// roster ↔ editor navigation under the page chrome.

import { useState } from "react";
import { FileText } from "lucide-react";

import { AppPage } from "@/components/app/page";
import { useAppRoutingContext } from "@/features/routing/RoutingContextProvider";
import { ReportsPage } from "./ReportsPage";
import { ReportEditor } from "./ReportEditor";

interface Props {
  /** The workspace; falls back to the routing context so the router can mount `<ReportsView/>` with no
   *  props (the sibling-surface convention). Tests / embeds may pass it explicitly. */
  ws?: string;
}

export function ReportsView({ ws: wsProp }: Props) {
  const ctx = useAppRoutingContext();
  const ws = wsProp ?? ctx.workspace;
  const [open, setOpen] = useState<string | null>(null);

  return (
    <AppPage
      label="reports"
      icon={FileText}
      title="Reports"
      description="Author branded, panel-bearing reports and export them as PDF."
      workspace={ws}
    >
      {open ? (
        <ReportEditor ws={ws} id={open} onClose={() => setOpen(null)} />
      ) : (
        <div className="flex min-w-0 flex-1 flex-col">
          <ReportsPage ws={ws} onOpen={setOpen} />
        </div>
      )}
    </AppPage>
  );
}
