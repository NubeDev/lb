// Result rows → the field-name list the editor's pickers offer (editor-parity scope, step 1). The
// preview's `viz.query` rows are the primary frame flattened to `Record<string, unknown>[]`, so the
// field names are the ordered union of the row keys. Pure — no React, no I/O (FILE-LAYOUT).

/** How many rows to scan for keys — enough to catch a sparse column without walking a huge result. */
const SCAN_ROWS = 50;

/** The ordered union of keys across the first rows (first-seen order — the frame's column order). */
export function fieldNamesOf(rows: Array<Record<string, unknown>>): string[] {
  const seen = new Set<string>();
  const names: string[] = [];
  for (const row of rows.slice(0, SCAN_ROWS)) {
    if (!row || typeof row !== "object") continue;
    for (const k of Object.keys(row)) {
      if (!seen.has(k)) {
        seen.add(k);
        names.push(k);
      }
    }
  }
  return names;
}
