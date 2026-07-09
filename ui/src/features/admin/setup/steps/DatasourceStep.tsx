// Shared setup-wizard step: pick (or register) a datasource (setup scope, rule-3 extraction). This is
// the datasource step BOTH the data→insight wizard and the render-template wizard use — one
// implementation, two homes (setup-wizards-scope "extract, don't fork"). It reuses the real
// `useDatasourceList` roster + `datasource.add`, the same roster the Datasources page uses, and
// registers the buildings demo idempotently if the workspace has none.
//
// One responsibility per file (FILE-LAYOUT): the datasource picker + demo-register affordance. The
// intro copy + StepShell framing stay in each wizard's flow file.

import { useState } from "react";
import { Check, Database, Loader2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import { addDatasource, listDatasources } from "@/lib/datasources";
import { useDatasourceList } from "@/features/panel-builder/tabs/useDatasourceList";
import { DEFAULT_SOURCE, DEMO_DSN, DEMO_ENDPOINT } from "../dataToInsight";

interface Props {
  ws: string;
  source: string;
  onPick: (name: string) => void;
  /** Cap-gate the demo-register button (display only — the gateway re-checks `datasource.add`). */
  canRegister: boolean;
}

/** The datasource step body — reuses the real `useDatasourceList` roster + `datasource.add`. Registers
 *  the buildings demo if the workspace has none (idempotent — a re-run just re-selects it). */
export function DatasourceStep({ ws, source, onPick, canRegister }: Props) {
  const { options, loading } = useDatasourceList(ws);
  const federation = options.filter((o) => o.type === "federation");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const registerDemo = async () => {
    setBusy(true);
    setError(null);
    try {
      const existing = await listDatasources();
      if (!existing.some((d) => d.name === DEFAULT_SOURCE)) {
        await addDatasource({ name: DEFAULT_SOURCE, kind: "sqlite", endpoint: DEMO_ENDPOINT, dsn: DEMO_DSN });
      }
      onPick(DEFAULT_SOURCE);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-3">
      {loading ? (
        <p className="flex items-center gap-2 text-xs text-muted">
          <Loader2 size={13} className="animate-spin" /> Loading datasources…
        </p>
      ) : federation.length > 0 ? (
        <label className="block space-y-1.5">
          <span className="text-xs font-medium text-muted">Registered datasources</span>
          {/* eslint-disable-next-line no-restricted-syntax -- a plain native select is the picker shape */}
          <select
            value={federation.some((o) => o.name === source) ? source : ""}
            onChange={(e) => onPick(e.target.value)}
            aria-label="Datasource"
            className="w-full rounded-md border border-border bg-bg px-3 py-2 text-sm text-fg"
          >
            <option value="" disabled>
              Choose a datasource…
            </option>
            {federation.map((o) => (
              <option key={o.name} value={o.name}>
                {o.label}
              </option>
            ))}
          </select>
        </label>
      ) : (
        <p className="text-xs text-muted">
          No datasources are registered yet. Register the buildings demo below to follow this wizard,
          or add one in the Datasources page first.
        </p>
      )}

      {canRegister && (
        <div className="flex flex-wrap items-center gap-2">
          <Button
            variant={federation.length > 0 ? "outline" : "default"}
            size="sm"
            disabled={busy}
            onClick={() => void registerDemo()}
            aria-label="Register the buildings demo datasource"
          >
            <Database size={13} /> {busy ? "Registering…" : "Register the buildings demo"}
          </Button>
          {source === DEFAULT_SOURCE && !busy && (
            <span className="inline-flex items-center gap-1 text-xs text-accent">
              <Check size={13} /> Using <span className="font-medium">{DEFAULT_SOURCE}</span>
            </span>
          )}
        </div>
      )}

      {error && (
        <p role="alert" className="text-xs text-red-500">
          {error}
        </p>
      )}
    </div>
  );
}
