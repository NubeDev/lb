// The in-cell placeholder for a widget that can't render — a dangling/unshared library panel, or a
// widget type this build doesn't support yet. An HONEST, legible state (product register: "empty states
// that teach, not 'nothing here'") rather than a bare line of muted text floating in an empty cell: a
// dimmed icon tile, a clear title, and one line of context. One component per file (FILE-LAYOUT).

import type { LucideIcon } from "lucide-react";

interface Props {
  icon: LucideIcon;
  title: string;
  detail?: string;
  /** `warn` tints the tile toward the warning tone (unsupported); default is a neutral muted tile. */
  tone?: "muted" | "warn";
  testId?: string;
}

export function WidgetPlaceholder({ icon: Icon, title, detail, tone = "muted", testId }: Props) {
  const tile =
    tone === "warn"
      ? "border-warning/25 bg-warning/10 text-warning"
      : "border-border bg-panel-2 text-muted";
  return (
    <div
      className="flex h-full flex-col items-center justify-center gap-2 px-4 text-center"
      role="status"
      data-testid={testId}
    >
      <span className={`flex h-9 w-9 items-center justify-center rounded-lg border ${tile}`}>
        <Icon size={17} />
      </span>
      <span className="text-xs font-medium text-fg">{title}</span>
      {detail && <span className="max-w-[22ch] text-[11px] leading-4 text-muted">{detail}</span>}
    </div>
  );
}
