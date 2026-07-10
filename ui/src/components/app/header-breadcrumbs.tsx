// HeaderBreadcrumbs — the alternative page-header style (shell-chrome-layout scope). A clean,
// minimal shadcn/ui `Breadcrumb` header: just the trail (`Workspace / <Surface>` for v1), the way
// shadcn renders breadcrumbs. The surface `icon` anchors the trail as a small muted glyph (a page
// affordance the plain trail lacked), never an accent chip or gradient wash. The trailing actions
// slot (workspace chip + Settings gear) is preserved so no surface loses an affordance by switching
// styles. Reached only when `theme.layout.header === "breadcrumbs"`; the band path (`AppPageHeader`)
// stays pixel-identical. One component per file (FILE-LAYOUT).

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
import { Button } from "@/components/ui/button";

import { WorkspaceBadge } from "./page-header";

interface HeaderBreadcrumbsProps {
  /** The surface glyph — rendered as a small muted anchor before the trail (parity with the band
   *  header's icon chip, without the chip). */
  icon: LucideIcon;
  title: string;
  workspace?: string;
  actions?: ReactNode;
}

export function HeaderBreadcrumbs({ icon: Icon, title, workspace, actions }: HeaderBreadcrumbsProps) {
  return (
    <header className="flex h-14 shrink-0 items-center gap-3 border-b border-border bg-bg px-4">
      <Icon size={16} className="shrink-0 text-muted" aria-hidden />
      <Breadcrumb>
        <BreadcrumbList className="gap-1.5 sm:gap-2">
          {workspace && (
            <>
              <BreadcrumbItem>
                <BreadcrumbLink
                  href={`#/t/${encodeURIComponent(workspace)}`}
                  className="text-muted hover:text-fg"
                >
                  {workspace}
                </BreadcrumbLink>
              </BreadcrumbItem>
              <BreadcrumbSeparator />
            </>
          )}
          <BreadcrumbItem>
            <BreadcrumbPage className="text-fg font-semibold tracking-tight">{title}</BreadcrumbPage>
          </BreadcrumbItem>
        </BreadcrumbList>
      </Breadcrumb>
      <div className="ml-auto flex items-center gap-2">
        {actions}
        {workspace && (
          <>
            <WorkspaceBadge workspace={workspace} />
            <span className="h-5 w-px bg-border" aria-hidden />
            <Button asChild variant="ghost" size="icon" className="h-8 w-8 text-muted hover:text-fg">
              <a
                href={`#/t/${encodeURIComponent(workspace)}/settings`}
                aria-label="Open settings"
                title="Settings"
              >
                <Settings size={15} />
              </a>
            </Button>
          </>
        )}
      </div>
    </header>
  );
}
