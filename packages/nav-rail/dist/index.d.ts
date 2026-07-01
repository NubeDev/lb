import { JSX } from 'react';
import type * as React_2 from 'react';

/** Internal: items bucketed by `group`, preserving first-seen group order. */
export declare interface NavGroup {
    /** undefined = the default, unlabeled group. */
    label?: string;
    items: NavItem[];
}

/** One entry in the rail. `icon` is a component (e.g. a lucide-react icon). */
export declare interface NavItem {
    /** Stable id echoed back through `onSelect`; also the active-match key. */
    id: string;
    label: string;
    /** Rendered at 16px; shown alone (with a tooltip) when the rail is collapsed. */
    icon?: React_2.ComponentType;
    /** Optional group heading. Items sharing a `group` render under one label, in array
     *  order; ungrouped items render in the default (unlabeled) group. */
    group?: string;
}

/**
 * An embedded vertical nav, self-themed like NavRail (`hsl(var(--nr-*))` under `.nav-rail`).
 * Ship the stylesheet with `import '@nube/nav-rail/style.css'`.
 */
export declare function NavMenu({ items, active, onSelect, badge, className, "aria-label": ariaLabel, }: NavMenuProps): JSX.Element;

export declare interface NavMenuProps {
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
 * A self-contained, self-themed sidebar. Wrap once at the app's left edge:
 *
 *   <NavRail items={items} active={sel} onSelect={setSel} header={<Brand/>} />
 *
 * Colors come from `hsl(var(--nr-*))` scoped to `.nav-rail`; override at `:root`, via
 * `className`, or inline `style` to re-skin without forking. Ship the stylesheet with
 * `import '@nube/nav-rail/style.css'`.
 */
export declare function NavRail({ items, active, onSelect, header, footer, defaultCollapsed, className, }: NavRailProps): React_2.JSX.Element;

export declare interface NavRailProps {
    /** The entries to show, in order. Group with `group`; gate by caps before passing in. */
    items: NavItem[];
    /** The selected item id (or null for none). Marked `aria-current="page"`. */
    active: string | null;
    /** Called with the clicked item's id. Routing/content is the host's job. */
    onSelect: (id: string) => void;
    /** Brand/logo area at the top; collapses with the rail. */
    header?: React_2.ReactNode;
    /** Footer area (e.g. a theme switcher or sign-out). */
    footer?: React_2.ReactNode;
    /** Start collapsed to icons. Default: expanded. */
    defaultCollapsed?: boolean;
    /** Extra classes on the `.nav-rail` root — a host theming hook. */
    className?: string;
}

export { }
