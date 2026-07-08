// The flow canvas primary controls (flow-deploy-ux scope; consolidated per flow-ui-polish scope —
// "less is more"). Presentational: every action is a prop; all state lives in the canvas/hooks.
//
// Idle the toolbar is exactly two controls; a run in flight adds one:
//   • Deploy    — `flows.save`. The ONLY path that pushes canvas edits to the running system. Enabled
//                 ONLY when the canvas differs from the deployed flow (`dirty`).
//   • Run ⇄ Stop — one morphing button: idle → `flows.run`; run active → `flows.cancel` (destructive).
//   • Pause ⇄ Resume — one toggle shown only mid-run (a run is either suspended or not; the old
//                 separate Suspend + Resume pair was pure noise). Keyed off `runStatus`.
//
// Enable/Disable, Live values, Undo, Export, Import, Delete live in the header's overflow menu
// (FlowOverflowMenu) — occasional operator actions, not every-minute controls.

import { Pause, Play, Save, Square } from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

export interface FlowToolbarProps {
  /** The canvas differs from the deployed flow → Deploy is enabled (and highlighted). */
  dirty: boolean;
  /** A run is in flight → Run morphs to Stop and the Pause/Resume toggle appears. */
  runActive: boolean;
  /** The watched run's status — `"suspended"` flips the Pause toggle to Resume. */
  runStatus: string | null;
  onDeploy: () => void;
  onRun: () => void;
  onLifecycle: (op: "suspend" | "resume" | "cancel") => void;
}

export function FlowToolbar({
  dirty,
  runActive,
  runStatus,
  onDeploy,
  onRun,
  onLifecycle,
}: FlowToolbarProps) {
  const suspended = runStatus === "suspended";
  return (
    <>
      <Button
        aria-label="deploy flow"
        onClick={onDeploy}
        disabled={!dirty}
        size="sm"
        className={cn("gap-1.5", dirty && "ring-2 ring-accent/40")}
        title={dirty ? "Deploy your changes to the running flow" : "No changes to deploy"}
      >
        <Save size={13} />
        {dirty ? "Deploy" : "Deployed"}
      </Button>
      {runActive ? (
        <>
          <Button
            aria-label="stop run"
            onClick={() => onLifecycle("cancel")}
            variant="destructive"
            size="sm"
            className="gap-1.5"
            title="Stop the active run"
          >
            <Square size={13} />
            Stop
          </Button>
          <Button
            aria-label={suspended ? "resume run" : "suspend run"}
            onClick={() => onLifecycle(suspended ? "resume" : "suspend")}
            variant="outline"
            size="sm"
            className="gap-1.5"
            title={suspended ? "Resume the suspended run" : "Pause the active run"}
          >
            {suspended ? <Play size={13} /> : <Pause size={13} />}
            {suspended ? "Resume" : "Pause"}
          </Button>
        </>
      ) : (
        <Button
          aria-label="run flow"
          onClick={onRun}
          variant="outline"
          size="sm"
          className="gap-1.5"
          title="Run this flow once now"
        >
          <Play size={13} />
          Run
        </Button>
      )}
    </>
  );
}
