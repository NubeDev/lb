// Unit test for the runtime arg widget + its `x-lb-widget:"runtime"` hint (external-agent
// run-lifecycle #5). NO real gateway — the `agent.runtimes` API seam is mocked so the render is
// deterministic; the real-path proof (a picked runtime posts a `kind:"agent"` item that settles to an
// answer) lives in the gateway test. Covers: the default is preselected; every configured id is an
// option; the schema hint the palette reads to select this widget.

import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { hintFor } from "@/lib/channel/palette.types";
import type { InputSchema } from "@/lib/channel/palette.types";

// Mock the one API seam the widget reads: two configured runtimes, default = "default".
vi.mock("@/lib/agent/runtimes.api", () => ({
  agentRuntimes: vi.fn(async () => ({
    default: "default",
    runtimes: ["acme-external", "default"],
  })),
}));

import { RuntimeArg } from "./RuntimeArg";

describe("RuntimeArg widget", () => {
  it("preselects the default id and lists every configured runtime", async () => {
    const onChange = vi.fn();
    render(<RuntimeArg value="" onChange={onChange} />);

    // The dropdown loads the configured runtimes as options.
    const select = (await screen.findByLabelText("runtime")) as HTMLSelectElement;
    await waitFor(() =>
      expect(within(select).queryAllByRole("option").length).toBe(2),
    );
    const optionValues = within(select)
      .getAllByRole("option")
      .map((o) => (o as HTMLOptionElement).value);
    expect(optionValues).toEqual(["acme-external", "default"]);

    // The default is preselected — the widget reports it up so an unset arg resolves to the default.
    await waitFor(() => expect(onChange).toHaveBeenCalledWith("default"));
    expect(select.value).toBe("default");
  });

  it("reports the picked runtime up on change", async () => {
    const onChange = vi.fn();
    render(<RuntimeArg value="default" onChange={onChange} />);
    const user = userEvent.setup();

    const select = (await screen.findByLabelText("runtime")) as HTMLSelectElement;
    await waitFor(() =>
      expect(within(select).queryAllByRole("option").length).toBe(2),
    );
    await user.selectOptions(select, "acme-external");
    expect(onChange).toHaveBeenCalledWith("acme-external");
  });
});

describe("the runtime widget schema hint", () => {
  it("hintFor reads x-lb:{widget:runtime} so the palette selects this widget", () => {
    const schema: InputSchema = {
      type: "object",
      properties: {
        goal: { type: "string" },
        runtime: { type: "string", "x-lb": { widget: "runtime" } },
      },
      required: ["goal"],
    };
    expect(hintFor(schema, "runtime")?.widget).toBe("runtime");
    expect(hintFor(schema, "goal")?.widget).toBeUndefined();
  });
});
