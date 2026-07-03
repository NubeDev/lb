// The editor's result-fields context (editor-parity scope, step 1): PanelEditor computes the draft's
// REAL field names ONCE (from the same `usePanelData` result the preview renders — one viz.query, no
// second fetch) and provides them here, so every tab's field picker offers the actual result fields
// without prop-drilling through each tab signature. One responsibility: carry `string[]` down the tree.

import { createContext, useContext, type ReactNode } from "react";

const FieldsContext = createContext<string[]>([]);

export function ResultFieldsProvider({ fields, children }: { fields: string[]; children: ReactNode }) {
  return <FieldsContext.Provider value={fields}>{children}</FieldsContext.Provider>;
}

/** The draft's current result field names (empty until the preview has frames). */
export function useResultFields(): string[] {
  return useContext(FieldsContext);
}
