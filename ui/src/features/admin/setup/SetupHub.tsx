// The Setup tab hub (setup scope) — the Setup tab's LANDING. It shows the wizard picker (icon-card
// grid over the catalog) until one is chosen, then swaps in that wizard with a back-to-picker crumb.
// This is the seam that lets the tab host more than one wizard: the grid is the menu, each entry
// resolves to a component here. One responsibility per file (FILE-LAYOUT): routing between picker and
// the active wizard; each wizard owns its own flow.

import { useState } from "react";
import { ChevronLeft } from "lucide-react";

import { Button } from "@/components/ui/button";
import { DashboardCacheProvider } from "@/features/dashboard/cache/DashboardQueryProvider";
import { AgentWizard } from "./AgentWizard";
import { AppearanceWizard } from "./AppearanceWizard";
import { DatasourceWizard } from "./DatasourceWizard";
import { IngestWizard } from "./IngestWizard";
import { TemplateWidgetWizard } from "./TemplateWidgetWizard";
import { WIZARDS, type WizardId } from "./catalog";
import { SetupWizard } from "./SetupWizard";
import { WizardPicker } from "./WizardPicker";

interface Props {
  ws: string;
  caps: string[] | undefined;
}

export function SetupHub({ ws, caps }: Props) {
  const [active, setActive] = useState<WizardId | null>(null);

  if (!active) {
    return <WizardPicker onPick={setActive} />;
  }

  const entry = WIZARDS.find((w) => w.id === active);

  return (
    <div className="flex h-full min-h-0 flex-col" data-testid="setup-hub">
      <div className="flex items-center gap-2 border-b border-border bg-panel-2/40 px-3 py-2">
        <Button
          variant="ghost"
          size="sm"
          className="h-7 gap-1 px-2 text-xs"
          onClick={() => setActive(null)}
          aria-label="Back to wizards"
        >
          <ChevronLeft size={14} /> Wizards
        </Button>
        {entry && <span className="text-xs font-medium text-muted">{entry.title}</span>}
      </div>
      <div className="min-h-0 flex-1">
        {active === "onboard" && <SetupWizard ws={ws} caps={caps} />}
        {active === "appearance" && (
          <AppearanceWizard ws={ws} caps={caps} onDone={() => setActive(null)} />
        )}
        {active === "ingest" && (
          <IngestWizard ws={ws} caps={caps} onDone={() => setActive(null)} />
        )}
        {active === "agent" && (
          <AgentWizard ws={ws} caps={caps} onDone={() => setActive(null)} />
        )}
        {active === "datasource" && (
          // The data→insight wizard renders live panel previews + the datasource roster, both of which
          // read through the shared dashboard query cache — mount it inside the same provider the
          // dashboard route uses so those reads resolve.
          <DashboardCacheProvider ws={ws}>
            <DatasourceWizard ws={ws} caps={caps} onDone={() => setActive(null)} />
          </DashboardCacheProvider>
        )}
        {active === "template" && (
          // The render-template wizard renders a live widget preview + the datasource roster, both of
          // which read through the shared dashboard query cache — same provider as the dashboard route.
          <DashboardCacheProvider ws={ws}>
            <TemplateWidgetWizard ws={ws} caps={caps} onDone={() => setActive(null)} />
          </DashboardCacheProvider>
        )}
      </div>
    </div>
  );
}
