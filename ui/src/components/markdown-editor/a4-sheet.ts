// The true-A4 print geometry the markdown editor sheet uses (reports scope, "true-A4 WYSIWYG editor").
// These are the SAME millimetre dimensions + margins the Typst PDF template uses, so on-screen matches
// print. Kept as plain constants (not a stylesheet) so the editor and the read/print view can share one
// source of truth for the page box. One responsibility: the A4 page-box measurements.

/** ISO A4, portrait. */
export const A4_WIDTH_MM = 210;
export const A4_HEIGHT_MM = 297;

/** The content margins the Typst template reserves (matches lazybones `pdf.rs`). */
export const A4_MARGIN_MM = 20;

/** Inline style for the "sheet" — a white A4 page with real mm dimensions + margins, centred on a grey
 *  desk. `minHeight` (not fixed height) lets content overflow past one page (the overflow indicator
 *  flags it) rather than clipping. */
export const a4SheetStyle: React.CSSProperties = {
  width: `${A4_WIDTH_MM}mm`,
  minHeight: `${A4_HEIGHT_MM}mm`,
  padding: `${A4_MARGIN_MM}mm`,
  margin: "0 auto",
  background: "#ffffff",
  color: "#111111",
  boxShadow: "0 1px 4px rgba(0,0,0,0.18), 0 8px 24px rgba(0,0,0,0.12)",
  boxSizing: "border-box",
};

/** The grey "desk" the sheet sits on. */
export const a4DeskStyle: React.CSSProperties = {
  background: "#6b7280",
  padding: "24px",
  overflow: "auto",
};
