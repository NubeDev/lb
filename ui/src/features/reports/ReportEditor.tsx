// ReportEditor — author a report's ordered blocks (reports scope, the builder). A two-pane surface:
// the block list on the left (add markdown/panel/image, move-up/down reorder, per-block controls) and
// the LIVE preview on the right (the real ReportView — widgets render for real, brand switch re-styles
// immediately). A report-level BrandPicker + range toolbar sit in the header; Save funnels through the
// one `report.save` verb (whole-record LWW + undo). Reorder is move-up/down (@dnd-kit is not a repo
// dep — NOTE); a simple, keyboard-accessible reorder is enough for v1. One responsibility: the block
// authoring surface (blocks.ts owns the array transforms; ReportView owns rendering).

import { useEffect, useMemo, useRef, useState } from "react";
import { ArrowDown, ArrowUp, Image as ImageIcon, LayoutPanelTop, Save, Trash2, Type } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { BrandPicker } from "@/components/brand-picker";
import { MarkdownEditor } from "@/components/markdown-editor";
import { getReport, saveReport, type Block, type Report } from "@/lib/report";
import { getBrand, type Brand } from "@/lib/brand";
import { readBrandImage } from "@/lib/branding";
import type { DashboardSearch } from "@/features/routing/search";
import { addBlock, emptyMarkdown, imageBlock, moveBlock, panelBlock, patchBlock, removeBlock } from "./blocks";
import { PanelPicker } from "./PanelPicker";
import { ReportView } from "./ReportView";
import { ExportButton } from "./ExportButton";

interface Props {
  ws: string;
  id: string;
  onClose: () => void;
}

const DEFAULT_RANGE: DashboardSearch = { from: "2026-07-01", to: "2026-07-10" };

export function ReportEditor({ ws, id, onClose }: Props) {
  const [report, setReport] = useState<Report | null>(null);
  const [title, setTitle] = useState("");
  const [blocks, setBlocks] = useState<Block[]>([]);
  const [brandId, setBrandId] = useState("");
  const [brand, setBrand] = useState<Brand | undefined>();
  const [range, setRange] = useState<DashboardSearch>(DEFAULT_RANGE);
  const [pickingPanel, setPickingPanel] = useState(false);
  const [error, setError] = useState<string | undefined>();
  const [saving, setSaving] = useState(false);
  const previewRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    let live = true;
    getReport(id)
      .then((r) => {
        if (!live) return;
        setReport(r);
        setTitle(r.title);
        setBlocks(r.blocks ?? []);
        setBrandId(r.brandId ?? "");
      })
      .catch((e) => live && setError(e instanceof Error ? e.message : String(e)));
    return () => {
      live = false;
    };
  }, [id]);

  // Resolve the chosen brand so the preview re-styles when it changes.
  useEffect(() => {
    if (!brandId) {
      setBrand(undefined);
      return;
    }
    let live = true;
    getBrand(brandId)
      .then((b) => live && setBrand(b))
      .catch(() => live && setBrand(undefined));
    return () => {
      live = false;
    };
  }, [brandId]);

  const preview = useMemo(() => ({ blocks }), [blocks]);

  async function onUploadImage(index: number, file: File) {
    try {
      // NOTE: v1 embeds the logo/image inline as a data-URI (the shipped branding-assets helper) rather
      // than the not-yet-wired `assets_put_asset` binary route (Track C gap). When that route lands the
      // upload should switch to an asset_id ref (≤8 MiB); inline stays the fallback for small images.
      const dataUri = await readBrandImage(file);
      setBlocks((b) => patchBlock(b, index, { assetId: dataUri }));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function save() {
    setSaving(true);
    setError(undefined);
    try {
      const saved = await saveReport(id, title, blocks, brandId, { range });
      setReport(saved);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {/* Header toolbar: title, brand, range, save/export. */}
      <div className="flex flex-wrap items-center gap-2 border-b border-border px-4 py-2">
        <Button variant="ghost" size="sm" onClick={onClose}>
          ← Reports
        </Button>
        <Input
          aria-label="report title"
          className="h-8 w-64 text-xs"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="Report title…"
        />
        <BrandPicker value={brandId} onChange={setBrandId} />
        <div className="flex items-center gap-1 text-xs text-muted">
          <Input
            aria-label="report range from"
            className="h-8 w-[8.5rem] text-xs"
            type="date"
            value={range.from}
            onChange={(e) => setRange((r) => ({ ...r, from: e.target.value }))}
          />
          <span>to</span>
          <Input
            aria-label="report range to"
            className="h-8 w-[8.5rem] text-xs"
            type="date"
            value={range.to}
            onChange={(e) => setRange((r) => ({ ...r, to: e.target.value }))}
          />
        </div>
        <div className="ml-auto flex items-center gap-2">
          <Button size="sm" disabled={saving || !title.trim()} onClick={() => void save()}>
            <Save size={13} /> {saving ? "Saving…" : "Save"}
          </Button>
          <ExportButton id={id} title={title} blocks={blocks} previewRef={previewRef} disabled={!report} />
        </div>
      </div>

      {error && (
        <div role="alert" className="border-b border-destructive/20 bg-destructive/10 px-4 py-2 text-sm text-destructive">
          {error}
        </div>
      )}

      <div className="flex min-h-0 flex-1">
        {/* Left: the block list + controls. */}
        <div className="flex w-[26rem] min-w-0 flex-col border-r border-border">
          <div className="flex flex-wrap items-center gap-1 border-b border-border px-3 py-2">
            <span className="mr-1 text-xs font-medium text-muted">Add block:</span>
            <Button size="sm" variant="outline" onClick={() => setBlocks((b) => addBlock(b, emptyMarkdown()))}>
              <Type size={12} /> Text
            </Button>
            <Button size="sm" variant="outline" onClick={() => setPickingPanel(true)}>
              <LayoutPanelTop size={12} /> Panel
            </Button>
            <Button size="sm" variant="outline" onClick={() => setBlocks((b) => addBlock(b, imageBlock("")))}>
              <ImageIcon size={12} /> Image
            </Button>
          </div>

          <div className="min-h-0 flex-1 overflow-auto p-3">
            {blocks.length === 0 ? (
              <p className="p-4 text-center text-xs text-muted">No blocks yet — add text, a panel, or an image.</p>
            ) : (
              <ul className="flex flex-col gap-2" data-testid="block-list">
                {blocks.map((block, i) => (
                  <li key={i} className="rounded-md border border-border bg-panel-2/40 p-2" data-testid={`block-${i}`}>
                    <div className="mb-1 flex items-center gap-1">
                      <span className="text-[10px] font-semibold uppercase tracking-wide text-muted">{block.kind}</span>
                      <div className="ml-auto flex items-center gap-0.5">
                        <IconBtn label={`move block ${i} up`} onClick={() => setBlocks((b) => moveBlock(b, i, -1))}>
                          <ArrowUp size={12} />
                        </IconBtn>
                        <IconBtn label={`move block ${i} down`} onClick={() => setBlocks((b) => moveBlock(b, i, 1))}>
                          <ArrowDown size={12} />
                        </IconBtn>
                        <IconBtn label={`remove block ${i}`} destructive onClick={() => setBlocks((b) => removeBlock(b, i))}>
                          <Trash2 size={12} />
                        </IconBtn>
                      </div>
                    </div>
                    <BlockControls block={block} index={i} onPatch={(p) => setBlocks((b) => patchBlock(b, i, p))} onUpload={onUploadImage} />
                  </li>
                ))}
              </ul>
            )}
          </div>
        </div>

        {/* Right: the live preview (the real ReportView). */}
        <div ref={previewRef} className="min-h-0 min-w-0 flex-1 overflow-auto">
          <ReportView report={preview} ws={ws} range={range} brand={brand} />
        </div>
      </div>

      {pickingPanel && (
        <PanelPicker
          ws={ws}
          onPick={(cell) => {
            setBlocks((b) => addBlock(b, panelBlock(cell)));
            setPickingPanel(false);
          }}
          onCancel={() => setPickingPanel(false)}
        />
      )}
    </div>
  );
}

/** Per-block controls: markdown body + page-break toggle, image caption/upload. (Panel blocks carry
 *  no inline control here — they're edited in the library.) */
function BlockControls({
  block,
  index,
  onPatch,
  onUpload,
}: {
  block: Block;
  index: number;
  onPatch: (patch: Partial<Block>) => void;
  onUpload: (index: number, file: File) => void;
}) {
  if (block.kind === "markdown") {
    return (
      <div className="flex flex-col gap-1">
        <MarkdownEditor value={block.body ?? ""} onChange={(md) => onPatch({ body: md })} label={`block ${index} markdown`} minRows={6} />
        <label className="flex items-center gap-1.5 text-xs text-muted">
          <input
            type="checkbox"
            aria-label={`page break before block ${index}`}
            checked={!!block.pageBreak}
            onChange={(e) => onPatch({ pageBreak: e.target.checked })}
          />
          Page break before
        </label>
      </div>
    );
  }
  if (block.kind === "image") {
    return (
      <div className="flex flex-col gap-1">
        <input
          type="file"
          aria-label={`upload image for block ${index}`}
          accept="image/*"
          className="text-xs"
          onChange={(e) => e.target.files?.[0] && onUpload(index, e.target.files[0])}
        />
        <Input
          aria-label={`caption for block ${index}`}
          className="h-7 text-xs"
          placeholder="Caption…"
          value={block.caption ?? ""}
          onChange={(e) => onPatch({ caption: e.target.value })}
        />
        <Input
          aria-label={`width for block ${index}`}
          className="h-7 text-xs"
          placeholder="Width (e.g. 100%)…"
          value={typeof block.width === "string" ? block.width : ""}
          onChange={(e) => onPatch({ width: e.target.value })}
        />
      </div>
    );
  }
  return <p className="truncate text-xs text-muted">{block.cell?.title ?? block.cell?.panelRef ?? "panel"}</p>;
}

function IconBtn({
  label,
  destructive,
  onClick,
  children,
}: {
  label: string;
  destructive?: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <Button
      aria-label={label}
      variant="ghost"
      size="icon"
      className={destructive ? "h-6 w-6 text-muted hover:text-destructive" : "h-6 w-6 text-muted hover:text-fg"}
      onClick={onClick}
    >
      {children}
    </Button>
  );
}
