// Organize-fields usability gate (editor-parity scope, step 3 headline): build an organize config
// entirely through the UI — reorder, hide, rename — over provided result fields, and assert the exact
// `organize` options (indexByName / excludeByName / renameByName) the backend reads. No JSON typed.

import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";

import { ResultFieldsProvider } from "../../fields/FieldsContext";
import { OrganizeEditor } from "./OrganizeEditor";

function Harness({ fields }: { fields: string[] }) {
  const [options, setOptions] = useState<Record<string, unknown>>({});
  return (
    <ResultFieldsProvider fields={fields}>
      <OrganizeEditor options={options} onChange={setOptions} />
      <output aria-label="json">{JSON.stringify(options)}</output>
    </ResultFieldsProvider>
  );
}

describe("OrganizeEditor", () => {
  it("renames, hides, and reorders real fields — writing the backend organize shape, no JSON", async () => {
    const user = userEvent.setup();
    render(<Harness fields={["time", "value", "host"]} />);

    // Rename `value` → "Temperature".
    await user.type(screen.getByLabelText("rename value"), "Temperature");
    // Hide `host`.
    await user.click(screen.getByLabelText("hide host"));
    // Move `value` up above `time`.
    await user.click(screen.getByLabelText("move up value"));

    const out = JSON.parse(screen.getByLabelText("json").textContent!);
    expect(out.renameByName).toEqual({ value: "Temperature" });
    expect(out.excludeByName).toEqual({ host: true });
    // After moving `value` up, the explicit order is value, time, host.
    expect(out.indexByName).toEqual({ value: 0, time: 1, host: 2 });
  });

  it("degrades to a prompt when there are no result fields yet", () => {
    render(<Harness fields={[]} />);
    expect(screen.getByText(/run the query/i)).toBeInTheDocument();
  });
});
