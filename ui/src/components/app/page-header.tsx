import type { ReactNode } from "react";
import type { LucideIcon } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

interface AppPageHeaderProps {
  icon: LucideIcon;
  title: string;
  description?: string;
  workspace?: string;
  actions?: ReactNode;
  className?: string;
}

function WorkspaceBadge({ workspace }: { workspace: string }) {
  return (
    <Badge variant="outline" className="max-w-[18rem] gap-1.5 rounded-full shadow-sm">
      <span className="h-1.5 w-1.5 rounded-full bg-accent" aria-hidden />
      <span className="truncate">{workspace}</span>
    </Badge>
  );
}

function AppPageHeader({
  icon: Icon,
  title,
  description,
  workspace,
  actions,
  className,
}: AppPageHeaderProps) {
  return (
    <header
      className={cn(
        "flex min-h-[3.75rem] items-center gap-3 border-b border-border bg-card/60 px-4 py-2.5",
        className,
      )}
    >
      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md border border-border bg-bg text-accent">
        <Icon size={16} />
      </div>
      <div className="min-w-0">
        <h1 className="truncate text-sm font-semibold text-fg">{title}</h1>
        {description && <p className="mt-0.5 truncate text-xs text-muted">{description}</p>}
      </div>
      <div className="ml-auto flex min-w-0 items-center gap-2">
        {actions}
        {workspace && <WorkspaceBadge workspace={workspace} />}
      </div>
    </header>
  );
}

export { AppPageHeader, WorkspaceBadge };
