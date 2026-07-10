// A tiny visual preview of a sidebar layout for the Layout-tab option cards — ported from the
// shadcn-store template's inline diagrams, repointed at the Lazybones base tokens (bg/panel/fg/
// border) so the previews track the live theme. Pure presentation, driven by the three layout axes.
// One component per file (FILE-LAYOUT).

import { cn } from "@/lib/utils";

import type { HeaderStyle, MenuAlign, NavMode, SidebarCollapsible, SidebarSide, SidebarVariant } from "@/lib/theme";

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

/** The header-style preview: `band` shows today's icon-chip + title strip; `breadcrumbs` shows a
 *  Workspace / Page crumb trail in the same strip. Both carry the same height so the cards align. */
export function HeaderDiagram({ header }: { header: HeaderStyle }) {
  return (
    <div className="flex h-12 items-center gap-2 rounded-md border border-border bg-bg px-2">
      {header === "band" ? (
        <>
          <div className="flex h-6 w-6 items-center justify-center rounded-md border border-accent/30 bg-accent/15">
            <div className="h-2.5 w-2.5 rounded-sm bg-accent/70" />
          </div>
          <div className="flex flex-col gap-0.5">
            <div className="h-1.5 w-12 rounded-sm bg-fg/70" />
            <div className="h-1 w-8 rounded-sm bg-fg/40" />
          </div>
        </>
      ) : (
        <>
          <div className="h-1.5 w-6 rounded-sm bg-fg/50" />
          <span className="text-muted">/</span>
          <div className="h-1.5 w-10 rounded-sm bg-fg/70" />
        </>
      )}
    </div>
  );
}

/** The nav-mode preview: `sidebar` shows a left rail beside a pane; `topmenu` shows a horizontal
 *  menu strip above a pane (no rail). */
export function NavDiagram({ nav }: { nav: NavMode }) {
  if (nav === "sidebar") {
    return (
      <div className="flex h-12 rounded-md border border-border bg-bg">
        <div className="flex w-5 flex-shrink-0 flex-col gap-1 border-r border-border bg-panel p-1">
          <div className="h-1 w-full rounded-sm bg-fg/60" />
          <div className="h-1 w-3/4 rounded-sm bg-fg/45" />
          <div className="h-1 w-2/3 rounded-sm bg-fg/35" />
        </div>
        <Pane />
      </div>
    );
  }
  return (
    <div className="flex h-12 flex-col gap-0 rounded-md border border-border bg-bg p-0">
      <div className="flex items-center gap-2 border-b border-border px-1.5 py-1">
        <div className="h-1.5 w-6 rounded-sm bg-fg/60" />
        <div className="h-1.5 w-5 rounded-sm bg-fg/45" />
        <div className="h-1.5 w-7 rounded-sm bg-fg/35" />
      </div>
      <Pane />
    </div>
  );
}

/** The menu-alignment preview (topmenu only): a floating menubar with the brand pinned left, the
 *  account chip pinned right, and the trigger group either hugging the brand (`start`) or centered. */
export function MenuAlignDiagram({ align }: { align: MenuAlign }) {
  const triggers = (
    <div className="flex items-center gap-1">
      <div className="h-1.5 w-4 rounded-sm bg-fg/55" />
      <div className="h-1.5 w-4 rounded-sm bg-fg/45" />
      <div className="h-1.5 w-4 rounded-sm bg-fg/35" />
    </div>
  );
  return (
    <div className="flex h-12 flex-col justify-center rounded-md border border-border bg-bg p-1.5">
      <div className="flex items-center gap-1.5 rounded-md border border-border bg-panel px-1.5 py-1">
        <div className="h-2 w-2 shrink-0 rounded-sm bg-accent/70" />
        <div className={cn("flex flex-1", align === "center" ? "justify-center" : "justify-start")}>{triggers}</div>
        <div className="h-2 w-2 shrink-0 rounded-full bg-fg/40" />
      </div>
    </div>
  );
}
