// The per-step transform debug view (editor-parity scope, step 7) — renders the pipeline's stepwise
// frames from `useVizSteps`: the input snapshot + the result AFTER each applied step, each as a small
// table (first frame, capped rows) so an author can SEE what a transform did to the data. Rides the same
// viz.query cap; a denied/empty result degrades honestly. One responsibility: show the stepwise frames.

import { useVizSteps, type StepFrame } from "@/features/dashboard/builder/useVizSteps";
import type { Cell } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { transformLabel } from "../transformRegistry";

const PREVIEW_ROWS = 8;

interface Props {
  /** The draft cell (its transformations[] is the pipeline the debug view steps through). */
  draft: Cell;
  scope?: VarScope;
  refreshKey?: number;
}

/** The first frame flattened to display rows (columns × up to PREVIEW_ROWS rows). */
function frameRows(frame: StepFrame | undefined): { cols: string[]; rows: unknown[][] } {
  if (!frame) return { cols: [], rows: [] };
  const cols = frame.fields.map((f) => f.name);
  const len = Math.min(frame.length ?? frame.fields[0]?.values.length ?? 0, PREVIEW_ROWS);
  const rows: unknown[][] = [];
  for (let i = 0; i < len; i++) rows.push(frame.fields.map((f) => f.values[i]));
  return { cols, rows };
}

export function TransformDebug({ draft, scope, refreshKey }: Props) {
  const { steps, loading, denied } = useVizSteps(draft, true, scope, refreshKey);
  const pipeline = draft.transformations ?? [];

  if (denied) return <p className="text-xs text-muted">No access to preview the transform steps.</p>;
  if (loading) return <p className="text-xs text-muted">running steps…</p>;
  if (steps.length === 0) return <p className="text-xs text-muted">No data to step through yet — run the query.</p>;

  return (
    <div className="grid gap-2" aria-label="transform debug">
      {steps.map((s, i) => {
        const { cols, rows } = frameRows(s.frames[0]);
        const label = s.step === null ? "Input" : transformLabel(pipeline[s.step]?.id ?? "");
        const frameCount = s.frames.length;
        return (
          <div key={i} className="rounded-md border border-border bg-bg p-2" aria-label={`step ${i} ${label}`}>
            <div className="mb-1 text-[11px] font-medium text-muted">
              {s.step === null ? "Input" : `After: ${label}`}
              {frameCount > 1 ? ` · ${frameCount} frames` : ""}
            </div>
            {cols.length === 0 ? (
              <p className="text-[11px] text-muted">no fields</p>
            ) : (
              <div className="overflow-x-auto">
                <table className="w-full text-[11px]">
                  <thead>
                    <tr className="text-left text-muted">
                      {cols.map((c) => (
                        <th key={c} className="px-1.5 py-0.5 font-mono font-normal">
                          {c}
                        </th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {rows.map((r, ri) => (
                      <tr key={ri} className="border-t border-border/50">
                        {r.map((v, ci) => (
                          <td key={ci} className="px-1.5 py-0.5 font-mono text-fg">
                            {v === null || v === undefined ? <span className="text-muted">·</span> : String(v)}
                          </td>
                        ))}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
