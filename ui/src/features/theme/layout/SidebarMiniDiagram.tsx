// A tiny visual preview of a sidebar layout for the Layout-tab option cards — ported from the
// shadcn-store template's inline diagrams, repointed at the Lazybones base tokens (bg/panel/fg/
// border) so the previews track the live theme. Pure presentation, driven by the three layout axes.
// One component per file (FILE-LAYOUT).

import { cn } from "@/lib/utils";

import type { SidebarCollapsible, SidebarSide, SidebarVariant } from "@/lib/theme";

/** Menu-line marks inside the rail. */
function Lines({ vertical = false }: { vertical?: boolean }) {
  return (
    <div className={cn("flex flex-1 flex-col gap-0.5 p-1", vertical && "items-center")}>
      <div className="h-0.5 w-full rounded-md bg-fg/60" />
      <div className="h-0.5 w-3/4 rounded-md bg-fg/50" />
      <div className="h-0.5 w-2/3 rounded-md bg-fg/40" />
      <div className="h-0.5 w-3/4 rounded-md bg-fg/30" />
    </div>
  );
}

/** The dashed content pane. */
function Pane() {
  return <div className="m-1 flex-1 rounded-sm border border-dashed border-muted/40 bg-bg/50" />;
}

export function VariantDiagram({ variant }: { variant: SidebarVariant }) {
  return (
    <div className={cn("flex h-12 rounded-md border border-border", variant === "inset" ? "bg-panel" : "bg-bg")}>
      <div
        className={cn(
          "flex w-3 flex-shrink-0 flex-col gap-0.5 bg-panel p-1",
          variant === "floating" && "m-1 rounded-md border-r border-border",
          variant === "inset" && "m-1 ms-0 rounded-md bg-panel/80",
          variant === "sidebar" && "border-r border-border",
        )}
      >
        <div className="h-0.5 w-full rounded-md bg-fg/60" />
        <div className="h-0.5 w-3/4 rounded-md bg-fg/50" />
        <div className="h-0.5 w-2/3 rounded-md bg-fg/40" />
      </div>
      <Pane />
    </div>
  );
}

export function CollapsibleDiagram({ collapsible }: { collapsible: SidebarCollapsible }) {
  return (
    <div className="flex h-12 rounded-md border border-border bg-bg">
      {collapsible === "offcanvas" ? (
        <div className="m-1 flex flex-1 items-center justify-start rounded-sm border border-dashed border-muted/40 bg-bg/50 pl-2">
          <div className="flex flex-col gap-0.5">
            <div className="h-0.5 w-3 rounded-md bg-fg/60" />
            <div className="h-0.5 w-3 rounded-md bg-fg/60" />
            <div className="h-0.5 w-3 rounded-md bg-fg/60" />
          </div>
        </div>
      ) : collapsible === "icon" ? (
        <>
          <div className="flex w-4 flex-shrink-0 flex-col items-center gap-1 border-r border-border bg-panel p-1">
            <div className="h-2 w-2 rounded-sm bg-fg/60" />
            <div className="h-2 w-2 rounded-sm bg-fg/40" />
            <div className="h-2 w-2 rounded-sm bg-fg/30" />
          </div>
          <Pane />
        </>
      ) : (
        <>
          <div className="flex w-6 flex-shrink-0 border-r border-border bg-panel">
            <Lines />
          </div>
          <Pane />
        </>
      )}
    </div>
  );
}

export function SideDiagram({ side }: { side: SidebarSide }) {
  const rail = (
    <div className={cn("flex w-6 flex-shrink-0 bg-panel", side === "left" ? "border-r border-border" : "border-l border-border")}>
      <Lines />
    </div>
  );
  return (
    <div className="flex h-12 rounded-md border border-border bg-bg">
      {side === "left" ? (
        <>
          {rail}
          <Pane />
        </>
      ) : (
        <>
          <Pane />
          {rail}
        </>
      )}
    </div>
  );
}
