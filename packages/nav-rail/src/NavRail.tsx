// NavRail — the reusable, data-driven sidebar. The generic descendant of lb's
// shell/NavRail: same look (collapsible-to-icon, grouped items, tooltips, header/footer
// slots, ⌘/Ctrl-B toggle) with every app concept removed. The host passes `items` +
// `active`/`onSelect` and optional `header`/`footer` nodes; this file is the whole public
// component (the shadcn engine underneath is internal).

import type * as React from "react";

import { groupItems, type NavItem } from "./items";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
  SidebarRail,
  SidebarTrigger,
} from "./primitives/sidebar";

export interface NavRailProps {
  /** The entries to show, in order. Group with `group`; gate by caps before passing in. */
  items: NavItem[];
  /** The selected item id (or null for none). Marked `aria-current="page"`. */
  active: string | null;
  /** Called with the clicked item's id. Routing/content is the host's job. */
  onSelect: (id: string) => void;
  /** Brand/logo area at the top; collapses with the rail. */
  header?: React.ReactNode;
  /** Footer area (e.g. a theme switcher or sign-out). */
  footer?: React.ReactNode;
  /** Start collapsed to icons. Default: expanded. */
  defaultCollapsed?: boolean;
  /** Extra classes on the `.nav-rail` root — a host theming hook. */
  className?: string;
}

/**
 * A self-contained, self-themed sidebar. Wrap once at the app's left edge:
 *
 *   <NavRail items={items} active={sel} onSelect={setSel} header={<Brand/>} />
 *
 * Colors come from `hsl(var(--nr-*))` scoped to `.nav-rail`; override at `:root`, via
 * `className`, or inline `style` to re-skin without forking. Ship the stylesheet with
 * `import '@nube/nav-rail/style.css'`.
 */
export function NavRail({
  items,
  active,
  onSelect,
  header,
  footer,
  defaultCollapsed = false,
  className,
}: NavRailProps) {
  const groups = groupItems(items);

  return (
    <SidebarProvider defaultOpen={!defaultCollapsed} className={`nav-rail ${className ?? ""}`}>
      <Sidebar collapsible="icon" variant="sidebar">
        <SidebarHeader>
          {header}
          {/* The collapse toggle is always present so the rail can be re-opened once
              collapsed, even when the host supplies no header. */}
          <div className="flex items-center justify-end px-1 group-data-[collapsible=icon]:justify-center">
            <SidebarTrigger aria-label="Toggle sidebar" title="Toggle sidebar" />
          </div>
        </SidebarHeader>

        <SidebarContent>
          {groups.map((group, i) => (
            <SidebarGroup key={group.label ?? `__default-${i}`}>
              {group.label && <SidebarGroupLabel>{group.label}</SidebarGroupLabel>}
              <SidebarGroupContent>
                <SidebarMenu>
                  {group.items.map((item) => {
                    const selected = active === item.id;
                    const Icon = item.icon;
                    return (
                      <SidebarMenuItem key={item.id}>
                        <SidebarMenuButton
                          aria-label={item.label}
                          aria-current={selected ? "page" : undefined}
                          isActive={selected}
                          tooltip={item.label}
                          onClick={() => onSelect(item.id)}
                        >
                          {Icon && <Icon />}
                          <span>{item.label}</span>
                        </SidebarMenuButton>
                      </SidebarMenuItem>
                    );
                  })}
                </SidebarMenu>
              </SidebarGroupContent>
            </SidebarGroup>
          ))}
        </SidebarContent>

        {footer && <SidebarFooter>{footer}</SidebarFooter>}
        <SidebarRail />
      </Sidebar>
    </SidebarProvider>
  );
}
