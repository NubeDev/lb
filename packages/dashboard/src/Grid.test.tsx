// The grid host: read-only vs editable chrome, registry dispatch inside the grid, and the
// drag/resize-stop → onLayout payload (driven through the same `mergeLayout` path the RGL
// callbacks call — jsdom can't synthesize real drags, so the pure merge carries that math and
// the render tests pin the chrome/mode behavior).

import { beforeAll, describe, expect, it, vi } from "vitest";
import { createEvent, fireEvent, render, screen } from "@testing-library/react";

// jsdom never lays out, so `offsetParent` is always null — which makes react-grid-layout's
// onDragStart bail before the drag begins. Shim it to the parent element (an ENVIRONMENT gap
// fill, not a fake of grid behavior: the real RGL/DraggableCore code path runs end to end).
beforeAll(() => {
  Object.defineProperty(HTMLElement.prototype, "offsetParent", {
    get() {
      return (this as HTMLElement).parentElement;
    },
  });
});

import type { Cell } from "./dashboard.types";
import { DashboardGrid } from "./Grid";
import { createRegistry } from "./registry";

const cell = (i: string, y: number, extra: Partial<Cell> = {}): Cell => ({
  i,
  x: 0,
  y,
  w: 6,
  h: 4,
  v: 2,
  widget_type: "chart",
  view: "stat",
  binding: { series: "s" },
  ...extra,
});

const row = (i: string, y: number, options?: Record<string, unknown>): Cell =>
  cell(i, y, { view: "row", w: 12, h: 1, title: i.toUpperCase(), options });

const reg = createRegistry().register("stat", ({ cell: c }) => <div>w:{c.i}</div>);

describe("DashboardGrid", () => {
  it("editable: shows drag handles + remove/duplicate chrome and mounts renderers", () => {
    render(
      <DashboardGrid
        cells={[cell("a", 0)]}
        editable
        registry={reg}
        onLayout={() => {}}
        onRemove={() => {}}
        onDuplicate={() => {}}
        stackBelow={0}
      />,
    );
    expect(screen.getByText("w:a")).toBeTruthy();
    expect(screen.getByLabelText("move cell a")).toBeTruthy();
    expect(screen.getByLabelText("remove cell a")).toBeTruthy();
    expect(screen.getByLabelText("duplicate cell a")).toBeTruthy();
  });

  it("read-only: renders the widget but NO drag handle and NO edit chrome", () => {
    render(
      <DashboardGrid
        cells={[cell("a", 0)]}
        editable={false}
        registry={reg}
        onLayout={() => {}}
        onRemove={() => {}}
        onDuplicate={() => {}}
        stackBelow={0}
      />,
    );
    expect(screen.getByText("w:a")).toBeTruthy();
    expect(screen.queryByLabelText("move cell a")).toBeNull();
    expect(screen.queryByLabelText("remove cell a")).toBeNull();
  });

  it("hides a collapsed row's members and fires the collapse/remove seams", () => {
    const onToggleRow = vi.fn();
    const onRemove = vi.fn();
    render(
      <DashboardGrid
        cells={[row("r", 0, { collapsed: true }), cell("m", 1)]}
        editable
        registry={reg}
        onLayout={() => {}}
        onRemove={onRemove}
        onToggleRow={onToggleRow}
        stackBelow={0}
      />,
    );
    expect(screen.queryByText("w:m")).toBeNull(); // collapsed member not rendered
    fireEvent.click(screen.getByLabelText("expand row R"));
    expect(onToggleRow).toHaveBeenCalledWith("r");
    fireEvent.click(screen.getByLabelText("remove cell r"));
    expect(onRemove).toHaveBeenCalledWith("r");
  });

  it("renders the honest placeholder inside the grid for an unregistered view", () => {
    render(
      <DashboardGrid
        cells={[cell("x", 0, { view: "gauge" })]}
        editable={false}
        registry={reg}
        onLayout={() => {}}
        stackBelow={0}
      />,
    );
    expect(screen.getByText(/No renderer for “gauge”/)).toBeTruthy();
  });

  it("shows the time-override badge so a shifted panel can't read as 'now'", () => {
    render(
      <DashboardGrid
        cells={[cell("t", 0, { queryOptions: { timeFrom: "6h", timeShift: "1d" } })]}
        editable={false}
        registry={reg}
        onLayout={() => {}}
        stackBelow={0}
      />,
    );
    expect(screen.getByText("Last 6h, 1d earlier")).toBeTruthy();
  });

  it("onLayout receives the FULL cells payload with merged geometry (the persistence seam)", () => {
    // Drive a REAL drag through react-grid-layout (mousedown on the handle → mousemove →
    // mouseup fires onDragStop → mergeLayout → onLayout). A moved collapsed row must carry its
    // hidden member in the SAME payload, and non-geometry fields must survive the merge.
    // `free` occupies y 0–4; the collapsed row `r` (hidden member `m` at y 5) sits below it.
    // Dragging `r` up past `free` reorders them — the row's y must change and `m` must shift by
    // the same Δy even though it never appeared in react-grid-layout's layout.
    const cells = [cell("free", 0, { w: 12 }), row("r", 4, { collapsed: true }), cell("m", 5)];
    const onLayout = vi.fn();
    render(
      <DashboardGrid cells={cells} editable registry={reg} onLayout={onLayout} stackBelow={0} />,
    );
    const handle = screen.getByLabelText("move cell r");
    fireEvent.mouseDown(handle, { clientX: 10, clientY: 300 });
    fireEvent.mouseMove(document, { clientX: 10, clientY: 5 }); // drag the row bar to the top
    fireEvent.mouseUp(document, { clientX: 10, clientY: 5 });
    expect(onLayout).toHaveBeenCalled();
    const payload: Cell[] = onLayout.mock.calls.at(-1)![0];
    expect(payload).toHaveLength(3); // the full record, not just the visible items
    const rY = payload.find((c) => c.i === "r")!.y;
    expect(rY).toBeLessThan(4); // the row actually moved up
    expect(payload.find((c) => c.i === "m")!.y).toBe(5 + (rY - 4)); // hidden member carried by Δy
    expect(payload.find((c) => c.i === "free")).toMatchObject({ view: "stat", w: 12, h: 4 }); // non-geometry kept
  });

  it("an EMPTY droppable grid is still a drop target (data-droppable marks it)", () => {
    // Regression: RGL sets an inline height from the row count, so a board with no cells is
    // 0px tall and `dragover` never reaches it — the FIRST drop onto a fresh page was
    // impossible. The stylesheet gives `[data-droppable]` a min-height; pin the attribute.
    const { container } = render(
      <DashboardGrid
        cells={[]}
        editable
        registry={reg}
        onLayout={() => {}}
        stackBelow={0}
        droppable
        droppingItem={{ i: "__drop__", w: 4, h: 3 }}
        onDrop={() => {}}
      />,
    );
    expect(container.querySelector(".lbdg-root")!.getAttribute("data-droppable")).toBe("true");
  });

  it("a read-only grid is NOT marked droppable", () => {
    const { container } = render(
      <DashboardGrid
        cells={[]}
        editable={false}
        registry={reg}
        onLayout={() => {}}
        stackBelow={0}
        droppable
        droppingItem={{ i: "__drop__", w: 4, h: 3 }}
        onDrop={() => {}}
      />,
    );
    expect(container.querySelector(".lbdg-root")!.getAttribute("data-droppable")).toBeNull();
  });

  it("droppable: an external drag-over + drop lands the slot on onDrop", () => {
    // Drive react-grid-layout's REAL external-drop path: a dragover on the grid creates the
    // dropping placeholder from `droppingItem`, the drop hands its landed slot to `onDrop`.
    // jsdom synthesizes dragover without clientX/clientY, so pin them on the native event (gap
    // fill, same spirit as the offsetParent shim above).
    const onDrop = vi.fn();
    const { container } = render(
      <DashboardGrid
        cells={[]}
        editable
        registry={reg}
        onLayout={() => {}}
        stackBelow={0}
        droppable
        droppingItem={{ i: "__drop__", w: 4, h: 3 }}
        onDrop={onDrop}
      />,
    );
    const grid = container.querySelector(".react-grid-layout")!;
    const over = createEvent.dragOver(grid);
    Object.defineProperty(over, "clientX", { value: 10 });
    Object.defineProperty(over, "clientY", { value: 10 });
    fireEvent(grid, over);
    fireEvent(grid, createEvent.drop(grid));
    expect(onDrop).toHaveBeenCalledTimes(1);
    const [slot] = onDrop.mock.calls[0];
    expect(slot).toMatchObject({ x: 0, y: 0, w: 4, h: 3 });
  });

  it("droppable is inert while read-only (no drop seam for viewers)", () => {
    const onDrop = vi.fn();
    const { container } = render(
      <DashboardGrid
        cells={[]}
        editable={false}
        registry={reg}
        onLayout={() => {}}
        stackBelow={0}
        droppable
        droppingItem={{ i: "__drop__", w: 4, h: 3 }}
        onDrop={onDrop}
      />,
    );
    const grid = container.querySelector(".react-grid-layout")!;
    const over = createEvent.dragOver(grid);
    Object.defineProperty(over, "clientX", { value: 10 });
    Object.defineProperty(over, "clientY", { value: 10 });
    fireEvent(grid, over);
    fireEvent(grid, createEvent.drop(grid));
    expect(onDrop).not.toHaveBeenCalled();
  });

  it("degrades to the read-only stack below the breakpoint", () => {
    // jsdom offsetWidth is 0 → the measured width stays at the 1200 fallback; force the stack
    // by setting the breakpoint above it.
    render(
      <DashboardGrid
        cells={[cell("a", 0)]}
        editable
        registry={reg}
        onLayout={() => {}}
        stackBelow={5000}
      />,
    );
    expect(screen.getByLabelText("dashboard stack")).toBeTruthy();
    expect(screen.queryByLabelText("move cell a")).toBeNull(); // the stack is read-only
  });
});
