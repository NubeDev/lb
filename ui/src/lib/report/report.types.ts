// The report wire shapes — mirror the gateway's `report.*` routes + the host `Report` record (reports
// scope). A report is a workspace asset made of an ordered `blocks[]` array (the notebook): markdown
// text, images (asset refs), and live dashboard panels. It carries a `brandId` (a reusable brand
// profile) and a report-level toolbar (the dashboard range/variable model, reused verbatim). Like a
// panel/dashboard it is a LENS: `report.get` gates the RECORD; every embedded panel's data re-checks
// under the VIEWER's caps at render, and the PDF export embeds only what the exporter could see.

import type { Cell } from "@/lib/dashboard";

/** The S4 asset-sharing visibility tiers (identical to a dashboard's / panel's). */
export type Visibility = "private" | "team" | "workspace";

/** The three block kinds, one envelope. Absent/optional fields are host-opaque additive fields
 *  (serde-defaulted server-side) — a byte-clean round-trip per kind:
 *   - `markdown` → `body` (GFM string) + `pageBreak`,
 *   - `image`    → `assetId` (into the shipped `assets.*` store) + `caption`/`width`,
 *   - `panel`    → `cell` (a v3 {@link Cell}: an inline spec OR a hydrated `panel:{id}` ref). */
export interface Block {
  kind: "markdown" | "image" | "panel";
  /** markdown body (GFM). */
  body?: string;
  /** image block: the `asset_id` into the shipped assets store. */
  assetId?: string;
  /** image block caption. */
  caption?: string;
  /** image block width (host-opaque; e.g. a percentage or a preset token). */
  width?: unknown;
  /** markdown block: force a page break before this block in the PDF (lazybones page semantics). */
  pageBreak?: boolean;
  /** panel block per-block options (e.g. a pinned range that wins for this block — decision 1). */
  options?: unknown;
  /** panel block: the renderable Cell (inline spec or hydrated `panel:{id}` ref). */
  cell?: Cell;
}

/** A full report record (blocks + brand + sharing metadata). */
export interface Report {
  id: string;
  title: string;
  owner: string;
  visibility: Visibility;
  blocks: Block[];
  /** The reusable brand profile id this report renders with (BrandPicker; never empty — host seeds
   *  one default). */
  brandId: string;
  /** The report-level range/variable toolbar (the dashboard `Toolbar`/`Variable` model, host-opaque
   *  here — flows to `WidgetHost` range/scope). */
  toolbar: unknown;
  schemaVersion?: number;
  updated_ts: number;
  deleted?: boolean;
}

/** The cheap roster row `report.list` returns (no blocks body). */
export interface ReportSummary {
  id: string;
  title: string;
  visibility: Visibility;
  updated_ts: number;
}

/** The client-captured panel snapshots sent with an export request, keyed by block index / `cell.i`.
 *  Each value is a base64 PNG (data-URI or raw base64) captured under the exporter's caps. */
export type ReportSnapshots = Record<string, string>;
