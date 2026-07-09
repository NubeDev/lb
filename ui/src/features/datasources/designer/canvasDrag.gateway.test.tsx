// Regression for "can't move the nodes, they are fixed" (schema-designer scope). The canvas was a
// fully-controlled React Flow whose `nodes` came from a `useMemo(record)` keyed on the table-set —
// so a position-only drag mirrored into `record.layout` but the memo never recomputed, and the node
// snapped back (read as "fixed"). The fix holds nodes in local `useState` (the FlowCanvas pattern),
// applies drag changes locally, and only re-seeds from the record when the table-set changes.
//
// jsdom can't measure layout well enough to simulate a real pointer drag, so this pins the two
// observable properties a snap-back regression would break: (1) each node renders AT its record
// layout position (the re-seed reads `layout`), and (2) React Flow's drag wiring is active on the
// node (the `draggable` class — a `nodesDraggable={false}` or static-position regression drops it).
// Uses the gateway config purely for its jsdom + ResizeObserver setup (no gateway call is made).

import { describe, expect, it } from "vitest";
import { render, cleanup } from "@testing-library/react";

import { SchemaDesignerCanvas } from "./SchemaDesignerCanvas";
import type { DbSchemaRecord } from "@/lib/datasources";

const record: DbSchemaRecord = {
  name: "shop",
  version: 1,
  tables: [
    { name: "site", pk: ["id"], columns: [{ name: "id", type: "integer", nullable: false }] },
    { name: "meter", pk: ["id"], columns: [{ name: "id", type: "integer", nullable: false }] },
  ],
  fks: [],
  layout: { site: { x: 123, y: 77 }, meter: { x: 456, y: 210 } },
};

function renderCanvas() {
  return render(
    <div style={{ width: 800, height: 600 }}>
      <SchemaDesignerCanvas
        record={record}
        onChange={() => {}}
        importSource={null}
        importing={false}
        onImportDone={() => {}}
      />
    </div>,
  );
}

describe("SchemaDesignerCanvas — nodes are movable (not fixed)", () => {
  it("renders each node at its record layout position", () => {
    const { container } = renderCanvas();
    const site = container.querySelector('[data-id="site"]') as HTMLElement;
    const meter = container.querySelector('[data-id="meter"]') as HTMLElement;
    expect(site.style.transform).toBe("translate(123px,77px)");
    expect(meter.style.transform).toBe("translate(456px,210px)");
    cleanup();
  });

  it("gives every node React Flow's drag wiring (the `draggable` class)", () => {
    const { container } = renderCanvas();
    const nodes = [...container.querySelectorAll(".react-flow__node")];
    expect(nodes).toHaveLength(2);
    for (const n of nodes) expect(n.className).toContain("draggable");
    cleanup();
  });
});
