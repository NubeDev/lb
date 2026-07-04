// The data inspector drawer (data-studio-ux scope, "make the loop honest" → Panel Inspect). Grafana's
// inspector, narrowed to what a panel author debugging a query needs:
//   - Data:  the effective rows as a scrollable grid (what the chart draws).
//   - JSON:  the raw frames the datasource returned + the shaped frames after the pipeline.
//   - Query: the RESOLVED request that ran — the interpolated targets (real SQL / tool+args), so the
//            author reads what actually executed, not the pre-`${var}` template. On an error, the request
//            is still shown so a bad query is visible.
// It is a pure view over the `SourceState` the preview already holds (`meta.inspect`); it fetches nothing.
// One responsibility: show one resolution's data/JSON/request.

import { useMemo, useState } from "react";
import { Copy } from "lucide-react";

import { Sheet, SheetContent, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";
import { Button } from "@/components/ui/button";
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from "@/components/ui/table";
import type { SourceState } from "@/features/dashboard/builder/useSource";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  state: SourceState;
}

/** Pretty-print a value; undefined → a short note (so an empty tab reads as "nothing here", not blank). */
function json(value: unknown, emptyNote: string): string {
  if (value === undefined) return emptyNote;
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <Button
      type="button"
      size="sm"
      variant="ghost"
      className="h-6 px-1.5 text-[11px]"
      onClick={() => {
        void navigator.clipboard?.writeText(text);
        setCopied(true);
        setTimeout(() => setCopied(false), 1200);
      }}
    >
      <Copy size={11} /> {copied ? "Copied" : "Copy"}
    </Button>
  );
}

/** A scrollable JSON block with a copy button. */
function JsonBlock({ value, emptyNote }: { value: unknown; emptyNote: string }) {
  const text = json(value, emptyNote);
  return (
    <div className="relative">
      <div className="absolute right-1 top-1 z-10">
        <CopyButton text={text} />
      </div>
      <pre className="max-h-[60vh] overflow-auto rounded-md border border-border bg-panel/60 p-2 text-[11px] leading-relaxed">
        {text}
      </pre>
    </div>
  );
}

/** The effective rows rendered as a grid (columns = the union of row keys, in first-seen order). */
function DataGrid({ rows }: { rows: Array<Record<string, unknown>> }) {
  const columns = useMemo(() => {
    const seen: string[] = [];
    for (const r of rows) for (const k of Object.keys(r)) if (!seen.includes(k)) seen.push(k);
    return seen;
  }, [rows]);

  if (rows.length === 0) {
    return <div className="p-3 text-xs text-muted">No rows.</div>;
  }
  return (
    <div className="max-h-[60vh] overflow-auto rounded-md border border-border">
      <Table>
        <TableHeader>
          <TableRow>
            {columns.map((c) => (
              <TableHead key={c} className="text-[11px]">
                {c}
              </TableHead>
            ))}
          </TableRow>
        </TableHeader>
        <TableBody>
          {rows.map((r, i) => (
            <TableRow key={i}>
              {columns.map((c) => (
                <TableCell key={c} className="text-[11px]">
                  {formatCell(r[c])}
                </TableCell>
              ))}
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  );
}

/** Render a cell value compactly (objects/arrays → JSON; scalars verbatim). */
function formatCell(v: unknown): string {
  if (v === null || v === undefined) return "";
  if (typeof v === "object") return JSON.stringify(v);
  return String(v);
}

export function DataInspector({ open, onOpenChange, state }: Props) {
  const inspect = state.meta?.inspect;
  const shaped = inspect?.shapedFrames;
  const [tab, setTab] = useState("data");
  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent side="right" className="flex w-full flex-col gap-3 p-4 sm:max-w-3xl">
        <SheetHeader>
          <SheetTitle className="text-sm">Inspect data</SheetTitle>
        </SheetHeader>
        <div className="flex items-center gap-2 text-[11px] text-muted">
          <span>{state.rows.length.toLocaleString()} rows</span>
          {state.meta?.frames !== undefined && <span>· {state.meta.frames} frames</span>}
          {state.meta?.ms !== undefined && <span>· {Math.round(state.meta.ms)} ms</span>}
          {state.meta?.error && <span className="text-danger">· {state.meta.error}</span>}
        </div>
        <Tabs value={tab} onValueChange={setTab} className="flex min-h-0 flex-1 flex-col">
          <TabsList>
            <TabsTrigger value="data">Data</TabsTrigger>
            <TabsTrigger value="json">JSON</TabsTrigger>
            <TabsTrigger value="query">Query</TabsTrigger>
          </TabsList>
          <TabsContent value="data" className="min-h-0 flex-1">
            <DataGrid rows={state.rows} />
          </TabsContent>
          <TabsContent value="json" className="min-h-0 flex-1 space-y-3">
            <div>
              <div className="mb-1 text-[11px] uppercase tracking-wide text-muted">Raw frames (pre-pipeline)</div>
              <JsonBlock value={inspect?.rawFrames} emptyNote="No frames captured." />
            </div>
            {shaped !== undefined && (
              <div>
                <div className="mb-1 text-[11px] uppercase tracking-wide text-muted">Shaped frames (post-pipeline)</div>
                <JsonBlock value={shaped} emptyNote="No shaped frames." />
              </div>
            )}
          </TabsContent>
          <TabsContent value="query" className="min-h-0 flex-1">
            <div className="mb-1 text-[11px] text-muted">
              The resolved request that ran — interpolated targets (the real SQL / tool + args).
            </div>
            <JsonBlock value={inspect?.request} emptyNote="No request (no source resolved yet)." />
          </TabsContent>
        </Tabs>
      </SheetContent>
    </Sheet>
  );
}
