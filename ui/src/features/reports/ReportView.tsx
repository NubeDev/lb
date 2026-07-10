// ReportView — the read / print-fidelity render of a report's blocks (reports scope). This IS the live
// preview: markdown blocks render on the same A4 sheet the editor uses, image blocks show their asset,
// and panel blocks render LIVE through {@link PanelEmbed} (the ONE shipped widget path — real data
// under the viewer's caps, no parallel renderer). "Sheets on a grey desk" print CSS; switching the
// brand re-styles immediately (the brand's palette drives the sheet accents). Each panel block carries
// a stable `data-testid`/`data-block-index` so the ExportButton can find its DOM node to snapshot.
// One responsibility: render an ordered block list as A4 sheets.

import { MarkdownBody, a4DeskStyle, a4SheetStyle } from "@/components/markdown-editor";
import { PanelEmbed } from "@/features/panel/PanelEmbed";
import type { Block, Report } from "@/lib/report";
import type { Brand } from "@/lib/brand";
import type { DashboardSearch } from "@/features/routing/search";

interface Props {
  report: Pick<Report, "blocks">;
  ws: string;
  /** The report-level range (a panel block may pin its own in `options` — decision 1). */
  range?: DashboardSearch;
  /** The resolved brand profile, so switching it re-styles the preview immediately. */
  brand?: Brand;
}

export function ReportView({ report, ws, range, brand }: Props) {
  // Brand accents drive the sheet (primary heading rule + text colour). Absent brand ⇒ neutral sheet.
  const sheetStyle: React.CSSProperties = {
    ...a4SheetStyle,
    ...(brand ? { color: brand.colors.text, background: brand.colors.background } : {}),
  };

  return (
    <div style={a4DeskStyle} className="min-h-0 flex-1" data-testid="report-view">
      {report.blocks.map((block, i) => (
        <div
          key={i}
          style={sheetStyle}
          className="mb-6"
          data-block-index={i}
          data-block-kind={block.kind}
        >
          <BlockBody block={block} index={i} ws={ws} range={range} brand={brand} />
        </div>
      ))}
    </div>
  );
}

function BlockBody({
  block,
  index,
  ws,
  range,
  brand,
}: {
  block: Block;
  index: number;
  ws: string;
  range?: DashboardSearch;
  brand?: Brand;
}) {
  if (block.kind === "markdown") {
    return <MarkdownBody label="report markdown">{block.body ?? ""}</MarkdownBody>;
  }
  if (block.kind === "image") {
    return (
      <figure className="m-0">
        {block.assetId ? (
          <img
            src={block.assetId.startsWith("data:") ? block.assetId : `/assets/${block.assetId}`}
            alt={block.caption ?? "report image"}
            className="max-w-full"
          />
        ) : (
          <div className="rounded border border-dashed border-black/20 p-6 text-center text-xs text-black/40">
            No image selected
          </div>
        )}
        {block.caption && <figcaption className="mt-1 text-center text-xs text-black/60">{block.caption}</figcaption>}
      </figure>
    );
  }
  // panel block — render the live widget through the shipped embed primitive.
  return (
    <div
      className="min-h-[16rem]"
      data-testid={`report-panel-${index}`}
      style={brand ? { accentColor: brand.colors.accent } : undefined}
    >
      {block.cell ? (
        <PanelEmbed ws={ws} cell={block.cell} range={range} />
      ) : (
        <div className="rounded border border-dashed border-black/20 p-6 text-center text-xs text-black/40">
          No panel selected
        </div>
      )}
    </div>
  );
}
