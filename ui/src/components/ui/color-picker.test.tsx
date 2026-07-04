// The hand-authored color-picker's interaction contract (theme-appearance scope, slice 0b). Proves the
// shipped bug is fixed: the WHOLE ROW opens the editor (not just a swatch), the editor is in-DOM (no
// native `<input type="color">`, which WebKitGTK/Tauri can't render), H/S/L + hex edits emit a valid
// triplet, and it is keyboard-operable and dismissible.

import { describe, expect, it } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";

import { ColorPicker } from "./color-picker";

function Harness({ initial = "32 92% 34%" }: { initial?: string }) {
  const [value, setValue] = useState(initial);
  return (
    <>
      <ColorPicker label="Accent" value={value} onChange={setValue} />
      <output aria-label="picked">{value}</output>
    </>
  );
}

const picked = () => screen.getByLabelText("picked").textContent;

describe("ColorPicker", () => {
  it("uses NO native color input (the WebKitGTK/Tauri no-op bug)", () => {
    const { container } = render(<Harness />);
    expect(container.querySelector('input[type="color"]')).toBeNull();
  });

  it("opens the in-DOM popover from a click anywhere on the row", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    // The trigger is the whole labelled row, not a 24px swatch.
    const row = screen.getByRole("button", { name: /Accent:/ });
    expect(screen.queryByRole("dialog")).toBeNull();
    await user.click(row);
    expect(screen.getByRole("dialog", { name: /Accent color/ })).toBeInTheDocument();
    // The three channel sliders are present and labelled.
    expect(screen.getByLabelText("Hue")).toBeInTheDocument();
    expect(screen.getByLabelText("Saturation")).toBeInTheDocument();
    expect(screen.getByLabelText("Lightness")).toBeInTheDocument();
  });

  it("emits a valid triplet when a channel changes", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: /Accent:/ }));
    const hue = screen.getByLabelText("Hue") as HTMLInputElement;
    // Range inputs: jsdom doesn't step a slider from keyboard, so drive the change directly (the range
    // is focusable + arrow-steppable in a real browser — the a11y affordance is native to `type=range`).
    fireEvent.change(hue, { target: { value: "200" } });
    expect(picked()).toBe("200 92% 34%");
  });

  it("accepts a hex value and converts it to a triplet", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: /Accent:/ }));
    const hex = screen.getByLabelText("Accent hex") as HTMLInputElement;
    await user.clear(hex);
    await user.type(hex, "#ffffff");
    await user.keyboard("{Enter}");
    // white → lightness 100, saturation 0.
    expect(picked()).toBe("0 0% 100%");
  });

  it("ignores an unparseable hex (fail-closed, no partial apply)", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: /Accent:/ }));
    const hex = screen.getByLabelText("Accent hex");
    await user.clear(hex);
    await user.type(hex, "not-a-color");
    await user.keyboard("{Enter}");
    expect(picked()).toBe("32 92% 34%"); // unchanged
  });

  it("closes on Escape", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: /Accent:/ }));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
