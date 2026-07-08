// The export/import dialog (flow-ui-polish scope) — Node-RED's transfer UX over the shipped JSON
// round-trip (flowTransfer.ts). Export: a monospace preview, pretty ⇄ compact, whole-flow vs
// selected-nodes scope (with a loud stripped-edges count), Copy + Download. Import: paste JSON or
// pick a file, a parsed node/edge count as confirmation, then Apply through the caller's real
// `flows.save` path. A modal is justified here (product register): the action is deliberate,
// bounded, and needs a preview surface the header can't host.

import { useMemo, useRef, useState } from "react";
import { Check, Copy, Download } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";
import type { Flow } from "@/lib/flows";
import { downloadFlowJson, flowToJson, parseFlowJson, strippedNeedsCount } from "./flowTransfer";

export type TransferTab = "export" | "import";

export interface FlowTransferDialogProps {
  flow: Flow;
  /** The canvas's current node selection (single-select today) — offers the "selected nodes" scope. */
  selectedIds: Set<string>;
  open: boolean;
  tab: TransferTab;
  onTabChange: (tab: TransferTab) => void;
  onClose: () => void;
  /** Apply an imported flow through the real save path; the host outcome renders inline. */
  onImport: (flow: Flow) => Promise<{ ok: boolean; error?: string }>;
}

export function FlowTransferDialog({
  flow,
  selectedIds,
  open,
  tab,
  onTabChange,
  onClose,
  onImport,
}: FlowTransferDialogProps) {
  const [pretty, setPretty] = useState(true);
  const [selectionOnly, setSelectionOnly] = useState(false);
  const [copied, setCopied] = useState(false);
  const [pasted, setPasted] = useState("");
  const [importError, setImportError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);

  const useSelection = selectionOnly && selectedIds.size > 0;
  const json = useMemo(
    () => flowToJson(flow, { pretty, selection: useSelection ? selectedIds : undefined }),
    [flow, pretty, useSelection, selectedIds],
  );
  const stripped = useMemo(
    () => (useSelection ? strippedNeedsCount(flow, selectedIds) : 0),
    [useSelection, flow, selectedIds],
  );

  // The paste buffer parsed live — the node/edge count is the "this is what you're importing" check.
  const parsed = useMemo(() => {
    if (pasted.trim() === "") return null;
    try {
      const f = parseFlowJson(pasted, flow);
      return { flow: f, nodes: f.nodes.length, edges: f.nodes.flatMap((n) => n.needs ?? []).length };
    } catch (e) {
      return { error: e instanceof Error ? e.message : String(e) };
    }
  }, [pasted, flow]);

  async function copy() {
    try {
      await navigator.clipboard.writeText(json);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // clipboard denied — the preview is selectable; nothing to surface
    }
  }

  async function apply() {
    if (!parsed || "error" in parsed) return;
    setBusy(true);
    setImportError(null);
    try {
      const res = await onImport(parsed.flow);
      if (res.ok) {
        setPasted("");
        onClose();
      } else {
        setImportError(res.error ?? "import failed");
      }
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent aria-label="flow transfer dialog" className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>Export / import flow</DialogTitle>
          <DialogDescription>
            The flow's JSON — connections are each node's <code>needs</code>; the{" "}
            <code>edges</code> list is informational.
          </DialogDescription>
        </DialogHeader>
        <Tabs value={tab} onValueChange={(v) => onTabChange(v as TransferTab)}>
          <TabsList>
            <TabsTrigger value="export">Export</TabsTrigger>
            <TabsTrigger value="import">Import</TabsTrigger>
          </TabsList>

          <TabsContent value="export" className="flex flex-col gap-2">
            <div className="flex flex-wrap items-center gap-3 text-xs text-fg">
              <label className="flex items-center gap-1.5">
                {/* eslint-disable-next-line no-restricted-syntax -- native checkbox, form-control vocabulary */}
                <input
                  type="checkbox"
                  aria-label="pretty print"
                  checked={pretty}
                  onChange={(e) => setPretty(e.target.checked)}
                />
                Pretty
              </label>
              <label className="flex items-center gap-1.5" title="Export only the selected node(s)">
                {/* eslint-disable-next-line no-restricted-syntax -- native checkbox, form-control vocabulary */}
                <input
                  type="checkbox"
                  aria-label="selected nodes only"
                  disabled={selectedIds.size === 0}
                  checked={useSelection}
                  onChange={(e) => setSelectionOnly(e.target.checked)}
                />
                Selected nodes only ({selectedIds.size})
              </label>
              {stripped > 0 ? (
                <span aria-label="stripped edges warning" className="text-amber-600 dark:text-amber-400">
                  {stripped} incoming wire{stripped === 1 ? "" : "s"} from outside the selection
                  stripped
                </span>
              ) : null}
            </div>
            <pre
              aria-label="export preview"
              className="max-h-72 overflow-auto rounded-md border border-border bg-bg/80 p-2 font-mono text-[11px] leading-snug text-fg"
            >
              {json}
            </pre>
            <div className="flex gap-2">
              <Button aria-label="copy flow json" onClick={() => void copy()} size="sm" className="gap-1.5">
                {copied ? <Check size={13} /> : <Copy size={13} />}
                {copied ? "Copied" : "Copy"}
              </Button>
              <Button
                aria-label="download flow json"
                onClick={() => downloadFlowJson(flow.id, json)}
                variant="outline"
                size="sm"
                className="gap-1.5"
              >
                <Download size={13} />
                Download
              </Button>
            </div>
          </TabsContent>

          <TabsContent value="import" className="flex flex-col gap-2">
            <Textarea
              aria-label="import json"
              placeholder="Paste a flow's JSON here…"
              value={pasted}
              onChange={(e) => setPasted(e.target.value)}
              className="max-h-72 min-h-40 font-mono text-[11px]"
            />
            <div className="flex flex-wrap items-center gap-2">
              <Button
                aria-label="apply import"
                onClick={() => void apply()}
                size="sm"
                disabled={busy || !parsed || "error" in parsed}
              >
                {parsed && !("error" in parsed)
                  ? `Import ${parsed.nodes} node${parsed.nodes === 1 ? "" : "s"} / ${parsed.edges} wire${parsed.edges === 1 ? "" : "s"}`
                  : "Import"}
              </Button>
              <Button
                aria-label="import from file"
                onClick={() => fileRef.current?.click()}
                variant="outline"
                size="sm"
              >
                From file…
              </Button>
              {/* eslint-disable-next-line no-restricted-syntax -- a hidden native file picker; no shadcn equivalent */}
              <input
                ref={fileRef}
                type="file"
                accept="application/json"
                className="hidden"
                onChange={(e) => {
                  const f = e.target.files?.[0];
                  if (f) void f.text().then(setPasted);
                  e.target.value = "";
                }}
              />
              <span className="text-xs text-muted">
                Importing replaces this flow's graph (re-validated by the host).
              </span>
            </div>
            {parsed && "error" in parsed ? (
              <span aria-label="import parse error" className="text-xs text-destructive">
                {parsed.error}
              </span>
            ) : null}
            {importError ? (
              <span aria-label="import error" className="text-xs text-destructive">
                {importError}
              </span>
            ) : null}
          </TabsContent>
        </Tabs>
      </DialogContent>
    </Dialog>
  );
}
