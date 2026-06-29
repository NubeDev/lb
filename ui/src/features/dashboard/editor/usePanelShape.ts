// The editor's result-shape probe (viz chart-types scope, "Result-shape ↔ type validation"). It reads
// the draft panel's rows through THE one data hook (`usePanelData` — invariant A; no separate fetch) and
// classifies the shape so the viz picker can offer only the views that shape honestly fills. One
// responsibility: draft cell → its data's `ResultShape`. The classification is `shape.ts`'s job.

import type { Cell } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";
import { usePanelData } from "../builder/usePanelData";
import { detectShape, type ResultShape } from "../views/shape";

/** The detected shape of the draft panel's current data (`unknown` while loading / on no data). */
export function usePanelShape(cell: Cell, scope: VarScope = emptyScope(), refreshKey = 0): ResultShape {
  const { rows, loading } = usePanelData(cell, scope, refreshKey);
  if (loading) return "unknown";
  return detectShape(rows);
}
