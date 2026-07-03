// The searchable-Select primitive's behavior contract (editor-parity scope, step 1): open →
// type-to-filter → pick; groups render; `allowCustom` commits free-typed text (the field-picker
// degrade path); Escape closes without committing.

import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";

import { Combobox, type ComboboxOption } from "./combobox";

const OPTIONS: ComboboxOption[] = [
  { value: "celsius", label: "°C", group: "Temperature", description: "degrees Celsius" },
  { value: "fahrenheit", label: "°F", group: "Temperature" },
  { value: "bytes", label: "B", group: "Data" },
];

function Harness({ allowCustom = false, clearable = false, initial = "" }: { allowCustom?: boolean; clearable?: boolean; initial?: string }) {
  const [value, setValue] = useState(initial);
  return (
    <>
      <Combobox aria-label="unit" options={OPTIONS} value={value} onChange={setValue} allowCustom={allowCustom} clearable={clearable} />
      <output aria-label="picked">{value}</output>
    </>
  );
}

describe("Combobox", () => {
  it("opens, filters by typed text, and commits the clicked option", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole("combobox", { name: "unit" }));
    await user.type(screen.getByLabelText("unit search"), "fahr");
    expect(screen.queryByText("°C")).not.toBeInTheDocument();
    await user.click(screen.getByText("°F"));
    expect(screen.getByLabelText("picked").textContent).toBe("fahrenheit");
    // the list closed after the pick
    expect(screen.queryByLabelText("unit search")).not.toBeInTheDocument();
  });

  it("matches on description text and renders group headers", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole("combobox", { name: "unit" }));
    expect(screen.getByText("Temperature")).toBeInTheDocument();
    expect(screen.getByText("Data")).toBeInTheDocument();
    await user.type(screen.getByLabelText("unit search"), "degrees");
    expect(screen.getByText("°C")).toBeInTheDocument();
    expect(screen.queryByText("B")).not.toBeInTheDocument();
  });

  it("allowCustom commits free-typed text via Enter (the no-frames degrade path)", async () => {
    const user = userEvent.setup();
    render(<Harness allowCustom />);
    await user.click(screen.getByRole("combobox", { name: "unit" }));
    await user.type(screen.getByLabelText("unit search"), "my_field{Enter}");
    expect(screen.getByLabelText("picked").textContent).toBe("my_field");
  });

  it("without allowCustom, unmatched text commits nothing and shows no matches", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole("combobox", { name: "unit" }));
    await user.type(screen.getByLabelText("unit search"), "zzz");
    expect(screen.getByText("no matches")).toBeInTheDocument();
    await user.keyboard("{Enter}");
    expect(screen.getByLabelText("picked").textContent).toBe("");
  });

  it("clearable shows a clear (×) only when a value is set, and clearing commits the empty value", async () => {
    const user = userEvent.setup();
    // No value → no clear affordance (nothing to clear).
    const empty = render(<Harness clearable />);
    expect(empty.queryByLabelText("clear unit")).not.toBeInTheDocument();
    empty.unmount();
    // A set value → the clear appears; clicking it resets to "" (— none —) without opening the list.
    render(<Harness clearable initial="celsius" />);
    expect(screen.getByRole("combobox", { name: "unit" })).toHaveTextContent("°C");
    await user.click(screen.getByLabelText("clear unit"));
    expect(screen.getByLabelText("picked").textContent).toBe("");
    expect(screen.queryByLabelText("unit search")).not.toBeInTheDocument();
  });

  it("Escape closes without committing", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole("combobox", { name: "unit" }));
    await user.type(screen.getByLabelText("unit search"), "celsius");
    await user.keyboard("{Escape}");
    expect(screen.queryByLabelText("unit search")).not.toBeInTheDocument();
    expect(screen.getByLabelText("picked").textContent).toBe("");
  });
});
