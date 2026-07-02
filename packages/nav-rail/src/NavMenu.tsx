// NavMenu — the embeddable sibling of NavRail. Same data model (items + active/onSelect,
// grouped by `group`) and the same token look, but rendered as a plain in-flow vertical
// menu: NO SidebarProvider, no position:fixed, no mobile Sheet, no collapse. Use it as
// SECTION navigation inside a panel/dialog (e.g. a Grafana-style panel editor's options
// rail) where the full app-shell NavRail's fixed positioning would escape the container.
//
// One responsibility: the in-flow menu. NavRail is the app-shell chrome; NavMenu is the
// embedded rail. Both are the package's public surface.

import { cn } from "./lib/cn";
import { groupItems, type NavItem } from "./items";

export interface NavMenuProps {
  items: NavItem[];
  active: string | null;
  onSelect: (id: string) => void;
  /** Optional trailing badge per item (e.g. a count on "Overrides"). */
  badge?: (id: string) => number | undefined;
  /** Extra classes on the `.nav-rail` root — a host theming hook. */
  className?: string;
  /** aria-label for the nav landmark. */
  "aria-label"?: string;
}

/**
 * An embedded vertical nav, self-themed like NavRail (`hsl(var(--nr-*))` under `.nav-rail`).
 * Ship the stylesheet with `import '@nube/nav-rail/style.css'`.
 */
export function NavMenu({
  items,
  active,
  onSelect,
  badge,
  className,
  "aria-label": ariaLabel = "section navigation",
}: NavMenuProps) {
  const groups = groupItems(items);

  return (
    <nav
      aria-label={ariaLabel}
      className={cn("nav-rail flex min-w-0 flex-col gap-2 text-nr-fg", className)}
    >
      {groups.map((group, i) => (
        <div key={group.label ?? `__default-${i}`} className="flex flex-col gap-1">
          {group.label && (
            <div className="px-2 text-xs font-medium text-nr-muted">{group.label}</div>
          )}
          {group.items.map((item) => {
            const selected = active === item.id;
            const Icon = item.icon;
            const count = badge?.(item.id);
            return (
              <button
                key={item.id}
                type="button"
                role="tab"
                aria-label={item.label}
                aria-current={selected ? "page" : undefined}
                aria-selected={selected}
                onClick={() => onSelect(item.id)}
                className={cn(
                  "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm outline-none ring-nr-accent transition-colors focus-visible:ring-2",
                  "[&>svg]:h-4 [&>svg]:w-4 [&>svg]:shrink-0",
                  selected
                    ? "bg-nr-bg font-medium text-nr-fg"
                    : "text-nr-muted hover:bg-nr-bg hover:text-nr-fg",
                )}
              >
                {Icon && <Icon />}
                <span className="min-w-0 flex-1 truncate">{item.label}</span>
                {count ? (
                  <span className="rounded-full bg-nr-accent/15 px-1.5 text-[10px] text-nr-accent">
                    {count}
                  </span>
                ) : null}
              </button>
            );
          })}
        </div>
      ))}
    </nav>
  );
}
