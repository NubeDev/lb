// The migrate dialog (schema-designer scope) — shows the planned DDL for a dry-run, and an Apply
// button for the admin to opt into `dry_run: false` (the Ask gate). Renders the destructive-refusal
// copy verbatim when the diff refuses (so the author knows what to do instead). shadcn-first.
// One responsibility, one file (FILE-LAYOUT).

import { useEffect, useState } from "react";
import { AlertTriangle, CheckCircle2, Loader2 } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Select } from "@/components/ui/select";
import { listDatasources, migrateSchema } from "@/lib/datasources";
import type { DbSchemaRecord, DatasourceSummary, MigrateResult } from "@/lib/datasources";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  schema: DbSchemaRecord;
  /** Called after a successful apply so the page can refresh. */
  onApplied: () => void;
}

/** The migrate dialog — pick a datasource, plan, review, apply. dry_run is the default; Apply sends
 *  `dry_run: false` (the explicit Ask-gate step). */
export function MigrateDialog({ open, onOpenChange, schema, onApplied }: Props) {
  const [sources, setSources] = useState<DatasourceSummary[]>([]);
  const [source, setSource] = useState<string>("");
  const [result, setResult] = useState<MigrateResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!open) return;
    setResult(null);
    setError(null);
    listDatasources()
      .then(setSources)
      .catch((e) => setError(e instanceof Error ? e.message : String(e)));
  }, [open]);

  const plan = async () => {
    if (!source) return;
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const r = await migrateSchema(source, schema, true);
      setResult(r);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const apply = async () => {
    if (!source) return;
    setBusy(true);
    setError(null);
    try {
      const r = await migrateSchema(source, schema, false);
      setResult(r);
      if (r.applied) onApplied();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>Migrate schema to a datasource</DialogTitle>
          <DialogDescription>
            Plan the additive DDL (CREATE TABLE + ADD COLUMN + ADD CONSTRAINT FK) against the live
            catalog, then apply. Destructive changes are refused with what-to-do-instead copy.
          </DialogDescription>
        </DialogHeader>

        <div className="flex flex-col gap-3">
          <label className="flex flex-col gap-1 text-xs font-medium text-fg">
            Target datasource
            <Select
              aria-label="target datasource"
              value={source}
              onChange={(e) => {
                setSource(e.target.value);
                setResult(null);
              }}
            >
              <option value="">— select —</option>
              {sources.map((s) => (
                <option key={s.name} value={s.name}>
                  {s.name} ({s.kind})
                </option>
              ))}
            </Select>
          </label>

          {error && (
            <Alert variant="destructive">
              <AlertTriangle size={14} />
              <AlertTitle>Migrate failed</AlertTitle>
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          )}

          {result?.destructiveRefusal && (
            <Alert>
              <AlertTriangle size={14} className="text-amber-500" />
              <AlertTitle>Refused — additive migrate only</AlertTitle>
              <AlertDescription className="whitespace-pre-wrap">
                {result.destructiveRefusal}
              </AlertDescription>
            </Alert>
          )}

          {result && !result.destructiveRefusal && (
            <div className="flex flex-col gap-1.5">
              <div className="flex items-center gap-2 text-xs">
                {result.applied ? (
                  <Badge variant="default" className="gap-1">
                    <CheckCircle2 size={11} /> applied
                  </Badge>
                ) : (
                  <Badge variant="outline">dry-run plan</Badge>
                )}
                <span className="text-muted">{result.statements.length} statement(s)</span>
              </div>
              <pre className="max-h-64 overflow-auto rounded-md border border-border bg-panel p-2 font-mono text-[11px] text-fg">
                {result.statements.map((s, i) => `${i + 1}. [${s.kind}] ${s.sql}`).join("\n\n")}
              </pre>
            </div>
          )}
        </div>

        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)} disabled={busy}>
            Close
          </Button>
          <Button variant="outline" onClick={plan} disabled={busy || !source}>
            {busy && result === null ? <Loader2 size={13} className="animate-spin" /> : null}
            Plan (dry-run)
          </Button>
          <Button
            variant="default"
            onClick={apply}
            disabled={busy || !source || !!result?.destructiveRefusal}
            title="Apply the plan (dry_run: false — the Ask gate)"
          >
            Apply
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
