// BudgetBadge — the run budget readout: wall-clock `ms` + the AI spend (calls/tokens), rules-workbench
// scope. One render component per concern (FILE-LAYOUT).

import type { AiBudget } from "@/lib/rules";

interface BudgetBadgeProps {
  ms: number;
  ai: AiBudget;
}

export function BudgetBadge({ ms, ai }: BudgetBadgeProps) {
  return (
    <div aria-label="budget" className="flex gap-3 text-xs text-muted">
      <span aria-label="budget ms">{ms} ms</span>
      <span aria-label="budget ai">
        ai: {ai.calls} calls / {ai.tokens} tokens
      </span>
    </div>
  );
}
