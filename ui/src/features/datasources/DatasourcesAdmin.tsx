// The Datasources admin page (rules-workbench scope, Phase 3) — the first-party shell surface over the
// shipped `datasource.*` host verbs (the federation extension is headless and ships NO UI). It is the
// add form (with the implied-grant display) + the roster (kind + endpoint + redacted secret ref) + a
// per-row connectivity probe (green/red) and remove. Trusted shell code driving the gateway, exactly as
// the dashboard surface drives `dashboard.*`. State lives in `useDatasources` (one refresh source); the
// gateway re-checks every cap server-side. One responsibility, one file (FILE-LAYOUT).

import { Database } from "lucide-react";

import { AdminPanel } from "@/features/admin/AdminPanel";
import { useDatasources } from "./useDatasources";
import { AddDatasourceForm } from "./AddDatasourceForm";
import { DatasourceRoster } from "./DatasourceRoster";

interface Props {
  ws: string;
}

export function DatasourcesAdmin({ ws }: Props) {
  const { sources, error, probes, add, remove, probe } = useDatasources();

  return (
    <AdminPanel icon={Database} title="Datasources" ws={ws} error={error}>
      <AddDatasourceForm onAdd={(input) => void add(input)} />
      <DatasourceRoster
        sources={sources}
        probes={probes}
        onTest={(name) => void probe(name)}
        onRemove={(name) => void remove(name)}
      />
    </AdminPanel>
  );
}
