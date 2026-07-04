import type { ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import { Settings } from "lucide-react";

import { cn } from "@/lib/utils";

interface AppPageHeaderProps {
  icon: LucideIcon;
  title: string;
  description?: string;
  workspace?: string;
  actions?: ReactNode;
  className?: string;
}

/** The workspace-wall chip — same voice as the StatusBar strip (mono, accent dot): the boundary is a
 *  fact, so it reads as data, not decoration. */
function WorkspaceBadge({ workspace }: { workspace: string }) {
  return (
    <span
      title={`Workspace ${workspace}`}
      className="inline-flex max-w-[18rem] items-center gap-1.5 rounded-full border border-border bg-bg/60 px-2.5 py-1 font-mono text-[11px] text-fg/80 shadow-sm"
    >
      <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-accent" aria-hidden />
      <span className="truncate">{workspace}</span>
    </span>
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
        "relative flex min-h-[3.75rem] items-center gap-3 bg-card/60 px-4 py-2.5",
        className,
      )}
    >
      {/* A committed accent wash rising from the title side — the header owns its band instead of
          being a grey strip with text in it. Low alpha; content contrast is untouched. */}
      <div
        aria-hidden
        className="pointer-events-none absolute inset-0"
        style={{
          background:
            "linear-gradient(90deg, hsl(var(--accent) / 0.09), hsl(var(--accent-2) / 0.04) 32%, transparent 60%)",
        }}
      />
      {/* The shell signature: a two-hue hairline (accent → secondary accent → neutral) instead of a
          flat border-b — one line that makes every page read as THIS product, not stock chrome. */}
      <div
        aria-hidden
        className="pointer-events-none absolute inset-x-0 bottom-0 h-px"
        style={{
          background:
            "linear-gradient(90deg, hsl(var(--accent) / 0.6), hsl(var(--accent-2) / 0.4) 34%, hsl(var(--border)) 72%)",
        }}
      />
      <div
        className="relative flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border text-accent"
        style={{
          borderColor: "hsl(var(--accent) / 0.25)",
          background:
            "linear-gradient(135deg, hsl(var(--accent) / 0.16), hsl(var(--accent-2) / 0.10))",
        }}
      >
        <Icon size={16} />
      </div>
      <div className="relative min-w-0">
        <h1 className="truncate text-[15px] font-semibold leading-tight tracking-tight text-fg">
          {title}
        </h1>
        {description && <p className="mt-0.5 truncate text-xs text-muted">{description}</p>}
      </div>
      <div className="relative ml-auto flex min-w-0 items-center gap-2">
        {actions}
        {workspace && (
          <>
            <WorkspaceBadge workspace={workspace} />
            <span className="h-5 w-px bg-border" aria-hidden />
            {/* Settings lives here now (top-right, where users look for it) — a plain hash link so
                this component stays router-free; the route gate re-checks caps regardless. */}
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

export { AppPageHeader, WorkspaceBadge };
