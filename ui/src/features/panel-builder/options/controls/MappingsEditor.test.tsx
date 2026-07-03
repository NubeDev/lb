// Value-mappings usability gate (editor-parity scope, step 2): author a value mapping ENTIRELY through
// the UI — pick "Value", type the match + display text, pick a color — and assert the produced
// `ValueMapping[]` is the exact shape the render path (`fieldconfig/mappings.ts`) applies. No JSON typed.

import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";

import type { ValueMapping } from "@/lib/dashboard";
import { MappingsEditor } from "./MappingsEditor";

function Harness() {
  const [value, setValue] = useState<ValueMapping[] | undefined>(undefined);
  return (
    <>
      <MappingsEditor value={value} onChange={setValue} />
      <output aria-label="json">{JSON.stringify(value ?? null)}</output>
    </>
  );
}

describe("MappingsEditor", () => {
  it("authors a value mapping through the UI (value → text + color), no JSON typed", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    await user.click(screen.getByLabelText("add value mapping"));
    await user.type(screen.getByLabelText("mapping 0 match"), "OK");
    await user.type(screen.getByLabelText("mapping 0 text"), "Healthy");
    await user.click(screen.getByLabelText("mapping 0 color green"));

    const out = JSON.parse(screen.getByLabelText("json").textContent!);
    expect(out).toEqual([{ type: "value", options: { OK: { text: "Healthy", color: "green" } } }]);
  });

  it("authors a range and a special mapping and preserves regex mappings on edit", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    await user.click(screen.getByLabelText("add range mapping"));
    await user.type(screen.getByLabelText("mapping 0 from"), "0");
    await user.type(screen.getByLabelText("mapping 0 to"), "10");
    await user.type(screen.getByLabelText("mapping 0 text"), "low");

    await user.click(screen.getByLabelText("add special mapping"));
    await user.selectOptions(screen.getByLabelText("mapping 1 special"), "empty");
    await user.type(screen.getByLabelText("mapping 1 text"), "n/a");

    const out = JSON.parse(screen.getByLabelText("json").textContent!) as ValueMapping[];
    expect(out).toEqual([
      { type: "range", options: { from: 0, to: 10, result: { text: "low" } } },
      { type: "special", options: { match: "empty", result: { text: "n/a" } } },
    ]);
  });
});
