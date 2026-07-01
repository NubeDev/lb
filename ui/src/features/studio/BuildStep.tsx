// Step 2 — confirm what you've got, then compile it. The inspection summary sits on top (identity,
// caps, toolchain readiness); the streaming terminal takes the rest of the height. The build itself is
// driven from the footer's primary action, which becomes "Continue" once an artifact exists.

import { InspectionSummary } from "./InspectionSummary";
import { BuildLog } from "./BuildLog";
import type { StudioWizard } from "./studio.wizard";

export function BuildStep({ w }: { w: StudioWizard }) {
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4">
      {w.inspect && <InspectionSummary inspect={w.inspect} />}
      <BuildLog logs={w.logs} streaming={w.busy} />
    </div>
  );
}
