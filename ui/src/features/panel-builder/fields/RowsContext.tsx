// The editor's result-ROWS context (template-prompt slice) — the sibling of `FieldsContext`: the
// builder pane provides the draft's REAL fetched rows (the same frames the preview renders, demo-
// swapped when demo is on) so deep option editors (the template AI-prompt copier) can embed actual
// data without prop-drilling or a second fetch. One responsibility: carry the rows down the tree.

import { createContext, useContext, type ReactNode } from "react";

type Row = Record<string, unknown>;

const RowsContext = createContext<Row[]>([]);

export function ResultRowsProvider({ rows, children }: { rows: Row[]; children: ReactNode }) {
  return <RowsContext.Provider value={rows}>{children}</RowsContext.Provider>;
}

/** The draft's current result rows (empty until the preview has frames). */
export function useResultRows(): Row[] {
  return useContext(RowsContext);
}
