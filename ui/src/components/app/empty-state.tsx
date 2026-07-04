// AppEmptyState — the canonical "select or create something" placeholder shown in a surface's body
// region before anything is open (Flows, Dashboard, …). A centered dashed-border card with an
// accent-tinted icon, a title, and a one-line hint. Extracted so every surface's empty state reads
// the same. `flex-1` so it fills the body region. One component per file (FILE-LAYOUT).

import type { LucideIcon } from "lucide-react";

interface AppEmptyStateProps {
  icon: LucideIcon;
  title: string;
  description: string;
}

export function AppEmptyState({ icon: Icon, title, description }: AppEmptyStateProps) {
  return (
    <div className="flex flex-1 items-center justify-center p-6">
      {/* A bare centered stack — no card box. On a large empty canvas a boxed card reads as a widget
          that failed to load; a quiet stack reads as an intentional resting state (Linear-style). */}
      <div className="flex max-w-sm flex-col items-center text-center">
        <div className="mb-4 flex h-10 w-10 items-center justify-center rounded-lg bg-fg/[0.05] text-muted shadow-[inset_0_0_0_1px_hsl(var(--fg)/0.07)]">
          <Icon size={18} strokeWidth={1.75} />
        </div>
        <p className="text-sm font-medium text-fg">{title}</p>
        <p className="mt-1.5 max-w-[36ch] text-xs leading-5 text-muted">{description}</p>
      </div>
    </div>
  );
}
