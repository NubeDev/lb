// Real-component tests for the reusable Panel + its section/table/KV pieces + resize.
// No fakes (CLAUDE §9): a real Radix-backed dialog, real DOM, real pointer/keyboard
// interaction. The panel portals into document.body — we query there.

import { fireEvent, render, screen, within } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import { Panel } from "./Panel";
import { Section } from "./Section";
import { PropTable } from "./PropTable";
import { KV } from "./KV";

/** A minimal host that drives open/close, exercising the real controlled contract. */
function Harness({ initialWidth }: { initialWidth?: number }) {
  const [open, setOpen] = useState(true);
  return (
    <>
      <button onClick={() => setOpen(true)}>reopen</button>
      <Panel
        open={open}
        onOpenChange={setOpen}
        title="Edit panel"
        description="One editor for add and edit."
        aria-label="test panel"
        initialWidth={initialWidth}
        footer={<button onClick={() => setOpen(false)}>Save</button>}
      >
        <div style={{ padding: 12 }}>
          <Section title="Properties (2)">
            <PropTable
              columns={[
                { key: "name", header: "name" },
                { key: "value", header: "value", ellipsize: true, maxWidth: 120 },
              ]}
              rows={[
                { id: "a", cells: { name: "speed", value: "42%" } },
                { id: "b", cells: { name: "status", value: "fault" }, tone: "warn" },
              ]}
            />
          </Section>
          <Section title="Metadata">
            <KV k="size" v="96 × 96" />
          </Section>
        </div>
      </Panel>
    </>
  );
}

// Radix's dialog Content takes its accessible name from the SheetTitle it wires as
// `aria-labelledby` (which wins over `aria-label`), so we query by the title text.
function panel() {
  return screen.getByRole("dialog", { name: "Edit panel" });
}

describe("Panel", () => {
  it("renders title, description, sections, dense table rows and KV", () => {
    render(<Harness />);
    const p = within(panel());
    expect(p.getByText("Edit panel")).toBeTruthy();
    expect(p.getByText("One editor for add and edit.")).toBeTruthy();
    expect(p.getByText("Properties (2)")).toBeTruthy();
    expect(p.getByText("speed")).toBeTruthy();
    expect(p.getByText("fault")).toBeTruthy();
    expect(p.getByText("96 × 96")).toBeTruthy();
  });

  it("is width-controlled and starts at initialWidth (not a fixed max-width)", () => {
    render(<Harness initialWidth={640} />);
    expect((panel() as HTMLElement).style.width).toBe("640px");
  });

  it("scopes the panel tokens via the .lb-panel root class on the surface", () => {
    render(<Harness />);
    expect(panel().className).toContain("lb-panel");
  });

  it("closes via the footer action (controlled onOpenChange)", () => {
    render(<Harness />);
    fireEvent.click(within(panel()).getByText("Save"));
    expect(screen.queryByRole("dialog", { name: "Edit panel" })).toBeNull();
  });

  it("resizes wider with ArrowLeft and narrower with ArrowRight on the handle", () => {
    render(<Harness initialWidth={640} />);
    const handle = within(panel()).getByRole("separator", { name: "resize panel" });
    fireEvent.keyDown(handle, { key: "ArrowLeft" }); // widen
    expect((panel() as HTMLElement).style.width).toBe("664px");
    fireEvent.keyDown(handle, { key: "ArrowRight" }); // narrow back
    expect((panel() as HTMLElement).style.width).toBe("640px");
  });

  it("clamps width to [minWidth, maxWidth]", () => {
    render(
      <Panel open onOpenChange={vi.fn()} title="t" aria-label="clamp panel" initialWidth={400} minWidth={360} maxWidth={420}>
        <div />
      </Panel>,
    );
    const p = () => screen.getByRole("dialog", { name: "t" });
    const handle = within(p()).getByRole("separator", { name: "resize panel" });
    // Step is 24; three ArrowLeft (=+72) would reach 472 but max is 420.
    fireEvent.keyDown(handle, { key: "ArrowLeft" });
    fireEvent.keyDown(handle, { key: "ArrowLeft" });
    fireEvent.keyDown(handle, { key: "ArrowLeft" });
    expect((p() as HTMLElement).style.width).toBe("420px");
  });

  it("widens on a left-drag of the handle (pointer)", () => {
    render(<Harness initialWidth={640} />);
    const handle = within(panel()).getByRole("separator", { name: "resize panel" });
    // jsdom has no PointerEvent (so `fireEvent.pointerDown({clientX})` would drop the
    // geometry) and no pointer-capture methods — stub capture and dispatch real
    // pointer-typed MouseEvents that DO carry clientX, so the hook's math runs for real.
    const target = handle as HTMLElement & {
      setPointerCapture: () => void;
      releasePointerCapture: () => void;
      hasPointerCapture: () => boolean;
    };
    target.setPointerCapture = vi.fn();
    target.releasePointerCapture = vi.fn();
    target.hasPointerCapture = () => true;
    const ptr = (type: string, clientX: number) =>
      new MouseEvent(type, { bubbles: true, clientX }) as unknown as PointerEvent;
    fireEvent(handle, Object.assign(ptr("pointerdown", 800), { pointerId: 1 }));
    fireEvent(handle, Object.assign(ptr("pointermove", 700), { pointerId: 1 })); // left 100 → +100 width
    fireEvent(handle, Object.assign(ptr("pointerup", 700), { pointerId: 1 }));
    expect((panel() as HTMLElement).style.width).toBe("740px");
  });
});
