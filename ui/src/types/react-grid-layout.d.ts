// Minimal ambient types for `react-grid-layout` (dashboard scope). The upstream package ships no
// `.d.ts` and the `@types` package is a deprecated stub, so we declare the narrow surface the Grid
// host uses (`react-grid-layout-scope`): the default `GridLayout` component + the `Layout` item shape.
// Kept intentionally small — only the props the dashboard grid passes.

declare module "react-grid-layout" {
  import type { ComponentType, ReactNode } from "react";

  /** One grid item's geometry (the cell layout the dashboard record mirrors 1:1). */
  export interface Layout {
    i: string;
    x: number;
    y: number;
    w: number;
    h: number;
    minW?: number;
    minH?: number;
    static?: boolean;
  }

  export interface GridLayoutProps {
    className?: string;
    layout?: Layout[];
    cols?: number;
    rowHeight?: number;
    width?: number;
    isDraggable?: boolean;
    isResizable?: boolean;
    draggableHandle?: string;
    draggableCancel?: string;
    onLayoutChange?: (layout: Layout[]) => void;
    onDragStop?: (layout: Layout[]) => void;
    onResizeStop?: (layout: Layout[]) => void;
    children?: ReactNode;
  }

  const GridLayout: ComponentType<GridLayoutProps>;
  export default GridLayout;
}
