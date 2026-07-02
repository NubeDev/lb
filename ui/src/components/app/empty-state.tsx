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
      <div className="flex max-w-sm flex-col items-center rounded-lg border border-dashed border-border bg-card/70 px-6 py-7 text-center shadow-sm shadow-black/5">
        <div className="mb-3 flex h-10 w-10 items-center justify-center rounded-md border border-border bg-bg text-accent">
          <Icon size={20} />
        </div>
        <p className="text-sm font-medium text-fg">{title}</p>
        <p className="mt-1 text-xs leading-5 text-muted">{description}</p>
      </div>
    </div>
  );
}
