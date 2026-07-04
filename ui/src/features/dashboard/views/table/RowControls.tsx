// The shared row-control column (widget-platform scope, Slice B) — the actions column a `table` cell
// renders when its `options.rowControls` is set. Each control drives a WRITE verb on the ROW object
// (pause a reminder, run it now, delete it). Extracted from the channel `ResponseTable` so the DASHBOARD
// `TablePanel` renders the SAME actions column for a pinned cell — a pinned reminder widget is fully
// interactive on the dashboard, identical to its channel render (the cross-surface fidelity invariant).
//
// One responsibility: render the actions column for one row. The row object IS the control's
// `VarScope.values` — `${id}`/`${enabled}` resolve from it via the shipped `interpolateArgs` engine
// (which matches `${name}`/`[[name]]`/`$name` — NOT `{{id}}`); `{{value}}` is the INTERACTION value
// (the switch bool). We do NOT extend the vars engine — the shipped `interpolateArgs` already substitutes
// named scope values; we just supply the row as the scope.
//
// `tools` is the cell's bridge leash (`cellTools(cell)` — source/action/sources tools); the host
// intersects it with the viewer's grant on every call (re-checked server-side).

import type { VarScope } from "@/lib/vars";
import { SwitchControl } from "../SwitchControl";
import { ButtonControl } from "../ButtonControl";

/** One per-row control declared in `options.rowControls` (the `x-lb-render` envelope field). `kind` picks
 *  the shipped control; `action` is the write `{ tool, argsTemplate }` (the template uses `${field}` for
 *  row fields, `{{value}}` for the interaction). `label`/`buttonLabel` are cosmetic. Mirrored from the
 *  channel `ResponseTable` (`RowControl`), and by the palette when it emits the envelope. */
export interface RowControl {
  kind: "switch" | "button";
  action: { tool: string; argsTemplate?: Record<string, unknown> };
  label?: string;
  buttonLabel?: string;
}

interface Props {
  /** The row object — used as the control's `VarScope.values` (`${field}` resolves from it). */
  row: Record<string, unknown>;
  controls: RowControl[];
  /** The cell's bridge leash (`cellTools(cell)`) — the forwardable tool set; host ∩ grant per call. */
  tools: string[];
}

/** Render the actions column for one row — a flex of SwitchControl/ButtonControl per declared control.
 *  Used by the dashboard `TablePanel` (a pinned cell) and the channel `ResponseTable` (a live response)
 *  so a pinned reminder widget is interactive on the dashboard exactly as it is in a channel. */
export function RowControls({ row, controls, tools }: Props) {
  const scope: VarScope = { values: row as VarScope["values"], builtins: {} };
  return (
    <div className="flex items-center gap-2">
      {controls.map((rc, j) =>
        rc.kind === "switch" ? (
          <SwitchControl
            key={j}
            action={rc.action}
            tools={tools}
            label={rc.label ?? ""}
            scope={scope}
          />
        ) : (
          <ButtonControl
            key={j}
            action={rc.action}
            tools={tools}
            options={{ buttonLabel: rc.buttonLabel ?? rc.label ?? "Run" }}
            label={rc.label ?? ""}
            scope={scope}
          />
        ),
      )}
    </div>
  );
}