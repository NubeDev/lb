// HeaderBreadcrumbs — the alternative page-header style (shell-chrome-layout scope). A clean,
// minimal shadcn/ui `Breadcrumb` header: just the trail (`Workspace / <Surface>` for v1), the way
// shadcn renders breadcrumbs — no icon chip, no description sub-line, no accent wash or gradient.
// The trailing actions slot (workspace chip + Settings gear) is preserved so no surface loses an
// affordance by switching styles. Reached only when `theme.layout.header === "breadcrumbs"`; the
// band path (`AppPageHeader`) stays pixel-identical. One component per file (FILE-LAYOUT).

import type { ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import { Settings } from "lucide-react";

import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";

import { WorkspaceBadge } from "./page-header";

interface HeaderBreadcrumbsProps {
  /** Prop-shape parity with `AppPageHeader`; unused in breadcrumb mode (the trail IS the title). */
  icon: LucideIcon;
  title: string;
  workspace?: string;
  actions?: ReactNode;
}

export function HeaderBreadcrumbs({ title, workspace, actions }: HeaderBreadcrumbsProps) {
  return (
    <header className="flex h-14 shrink-0 items-center gap-2 border-b bg-background px-4">
      <Breadcrumb>
        <BreadcrumbList>
          {workspace && (
            <>
              <BreadcrumbItem>
                <BreadcrumbLink className="text-muted-foreground">{workspace}</BreadcrumbLink>
              </BreadcrumbItem>
              <BreadcrumbSeparator />
            </>
          )}
          <BreadcrumbItem>
            <BreadcrumbPage>{title}</BreadcrumbPage>
          </BreadcrumbItem>
        </BreadcrumbList>
      </Breadcrumb>
      <div className="ml-auto flex items-center gap-2">
        {actions}
        {workspace && (
          <>
            <WorkspaceBadge workspace={workspace} />
            <span className="h-5 w-px bg-border" aria-hidden />
            <a
              href={`#/t/${encodeURIComponent(workspace)}/settings`}
              aria-label="Open settings"
              title="Settings"
              className="icon-button"
            >
              <Settings size={15} />
            </a>
          </>
        )}
      </div>
    </header>
  );
}
