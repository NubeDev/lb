// ExportButton — the snapshot pass + PDF download (reports scope, "Panels export as client snapshots").
// For each PANEL block it finds the rendered DOM node (`data-testid="report-panel-{i}"`, inside a live
// ReportView), captures it to a PNG under the VIEWER's caps via `captureBlock`, collects the snapshots
// keyed by block index, then `exportReport(id, snapshots)` → downloads the returned PDF Blob as
// `{slug}.pdf`. The server never fetches widget data for export — the PDF holds only what the exporter
// could see. An uncapturable widget (sandboxed extension tier) is skipped, honestly, rather than
// failing the whole export (the node renders a titled placeholder for a missing snapshot). One
// responsibility: drive the export handshake from a live preview.

import { useState } from "react";
import { FileDown } from "lucide-react";

import { Button } from "@/components/ui/button";
import { captureBlock } from "@/lib/snapshot";
import { exportReport, type Block, type ReportSnapshots } from "@/lib/report";

interface Props {
  id: string;
  title: string;
  blocks: Block[];
  /** The live-preview container to scan for rendered panel nodes (a ref to ReportView's root). */
  previewRef: React.RefObject<HTMLElement | null>;
  disabled?: boolean;
}

export function ExportButton({ id, title, blocks, previewRef, disabled }: Props) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | undefined>();

  async function run() {
    setBusy(true);
    setError(undefined);
    try {
      const snapshots: ReportSnapshots = {};
      const root = previewRef.current;
      for (let i = 0; i < blocks.length; i++) {
        if (blocks[i].kind !== "panel") continue;
        const node = root?.querySelector<HTMLElement>(`[data-testid="report-panel-${i}"]`);
        if (!node) continue;
        try {
          snapshots[String(i)] = await captureBlock(node);
        } catch {
          // Uncapturable widget → skip; the node substitutes a titled placeholder for the missing snap.
        }
      }
      const blob = await exportReport(id, snapshots);
      downloadBlob(blob, `${slug(title)}.pdf`);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex items-center gap-2">
      {error && <span className="text-xs text-destructive">{error}</span>}
      <Button size="sm" variant="outline" disabled={disabled || busy} onClick={() => void run()}>
        <FileDown size={13} /> {busy ? "Exporting…" : "Export PDF"}
      </Button>
    </div>
  );
}

function downloadBlob(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
}

function slug(title: string): string {
  return (
    title
      .toLowerCase()
      .trim()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "") || "report"
  );
}
