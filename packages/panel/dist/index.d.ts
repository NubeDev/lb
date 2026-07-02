import { JSX as JSX_2 } from 'react';
import { NavItem } from '@nube/nav-rail';
import { NavMenu } from '@nube/nav-rail';
import { NavMenuProps } from '@nube/nav-rail';
import { ReactNode } from 'react';

/** A dense key/value row — the ce InspectPanel `KV` look on shadcn tokens. */
export declare function KV({ k, v, keyWidth, className }: KVProps): JSX_2.Element;

export declare interface KVProps {
    k: ReactNode;
    v: ReactNode;
    /** Key-column width in px (ce uses 80). */
    keyWidth?: number;
    className?: string;
}

export { NavItem }

export { NavMenu }

export { NavMenuProps }

/** The reusable resizable side panel — ce InspectPanel look on shadcn primitives. */
export declare function Panel({ open, onOpenChange, title, description, headerAside, footer, "aria-label": ariaLabel, initialWidth, minWidth, maxWidth, className, children, }: PanelProps): JSX_2.Element;

export declare interface PanelProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    /** The panel heading. */
    title: ReactNode;
    /** Sub-heading under the title (optional). */
    description?: ReactNode;
    /** Trailing controls on the header row (e.g. a status chip). */
    headerAside?: ReactNode;
    /** The action row pinned to the bottom (e.g. Cancel / Save). Omit for a footer-less panel. */
    footer?: ReactNode;
    /** aria-label on the dialog content. */
    "aria-label"?: string;
    /** Initial width in px. Default 720 — roomy, unlike the old cramped `sm:max-w-3xl`. */
    initialWidth?: number;
    /** Min width in px (default 360). */
    minWidth?: number;
    /** Max width in px (default 1200 — wide enough to reveal every option column). */
    maxWidth?: number;
    /** Extra classes on the docked surface. */
    className?: string;
    /** The scrollable body — the host stacks <Section>/<PropTable>/<KV> here. */
    children: ReactNode;
}

export declare interface PropColumn {
    /** Column key — also the header text unless `header` is given. */
    key: string;
    header?: ReactNode;
    /** Truncate + ellipsize the cell (with a title tooltip). For long value cells. */
    ellipsize?: boolean;
    /** Fixed max width in px for the cell (pairs with ellipsize). */
    maxWidth?: number;
    className?: string;
}

export declare interface PropRow {
    /** Stable row key. */
    id: string;
    /** Cell content per column key. */
    cells: Record<string, ReactNode>;
    /** Optional per-row emphasis (e.g. a fault/override row). */
    tone?: "default" | "warn";
}

/** A dense, monospace property table — the ce InspectPanel look on shadcn tokens. */
export declare function PropTable({ columns, rows, empty, className }: PropTableProps): JSX_2.Element;

export declare interface PropTableProps {
    columns: PropColumn[];
    rows: PropRow[];
    /** Shown when rows is empty. */
    empty?: ReactNode;
    className?: string;
}

export declare interface Resizable {
    /** Current width in px. */
    width: number;
    /** Whether a drag is in progress (host can dim/limit repaint). */
    dragging: boolean;
    /** Spread onto the drag handle element. */
    handleProps: {
        onPointerDown: (e: React.PointerEvent) => void;
        onPointerMove: (e: React.PointerEvent) => void;
        onPointerUp: (e: React.PointerEvent) => void;
        onKeyDown: (e: React.KeyboardEvent) => void;
    };
}

/** The Panel's left-edge drag-to-resize grabber. */
export declare function ResizeHandle({ resizable, className, "aria-label": ariaLabel }: ResizeHandleProps): JSX_2.Element;

export declare interface ResizeHandleProps {
    resizable: Resizable;
    className?: string;
    "aria-label"?: string;
}

/** A titled, dense group — the ce InspectPanel `Section` look on shadcn tokens. */
export declare function Section({ title, aside, className, children }: SectionProps): JSX_2.Element;

export declare interface SectionProps {
    /** The uppercase group label (e.g. "Properties (12)"). */
    title: ReactNode;
    /** Optional trailing controls on the header row (a button, a count, a toggle). */
    aside?: ReactNode;
    className?: string;
    children: ReactNode;
}

/** Controls a right-docked panel's width via a left-edge drag handle. */
export declare function useResizable({ initial, min, max, step }: UseResizableOptions): Resizable;

export declare interface UseResizableOptions {
    initial: number;
    min: number;
    max: number;
    /** Arrow-key step for keyboard resize (accessibility). */
    step?: number;
}

export { }
