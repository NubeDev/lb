// The Datasources admin page (rules-workbench scope, Phase 3) — the first-party shell surface over the
// shipped `datasource.*` host verbs (the federation extension is headless and ships NO UI). It is the
// add dialog (with the implied-grant display) + the roster (kind + endpoint + redacted secret ref) + a
// per-row connectivity probe (green/red) and remove. Trusted shell code driving the gateway, exactly as
// the dashboard surface drives `dashboard.*`. State lives in `useDatasources` (one refresh source); the
// gateway re-checks every cap server-side. One responsibility, one file (FILE-LAYOUT).
//
// Wraps in the canonical `AppPage` shell so the page reads like the Dashboards/Rules/Webhooks surfaces
// (accent header, workspace chip, settings link, page-transition Reveal). The "New datasource" action
// lives in the header's `actions` slot and opens a focused Dialog (matches the Webhooks "New webhook"
// action-in-header pattern); the roster fills the body.

import { Database } from "lucide-react";

import { AppPage } from "@/components/app/page";
import { useDatasources } from "./useDatasources";
import { AddDatasourceDialog } from "./AddDatasourceDialog";
import { DatasourceRoster } from "./DatasourceRoster";

interface Props {
  ws: string;
  onOpen: (name: string) => void;
}

export function DatasourcesAdmin({ ws, onOpen }: Props) {
  const { sources, error, probes, add, remove, probe } = useDatasources();

  return (
    <AppPage
      label="datasources admin"
      icon={Database}
      title="Datasources"
      description="Register external sources the workspace can query."
      workspace={ws}
      error={error}
      actions={<AddDatasourceDialog onAdd={(input) => void add(input)} />}
    >
      <div className="min-h-0 flex-1 overflow-y-auto">
        <DatasourceRoster
          sources={sources}
          probes={probes}
          onOpen={onOpen}
          onTest={(name) => void probe(name)}
          onRemove={(name) => void remove(name)}
        />
      </div>
    </AppPage>
  );
}
