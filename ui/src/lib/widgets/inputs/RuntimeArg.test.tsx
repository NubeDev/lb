// Unit test for the runtime arg widget + its `x-lb-widget:"runtime"` hint (external-agent
// run-lifecycle #5). NO real gateway — the `agent.runtimes` API seam is mocked so the render is
// deterministic; the real-path proof (a composer that never touches the dropdown posts NO `runtime`,
// so the workspace's active pick wins at the host) lives in the gateway test. Covers: on mount with
// no value the widget does NOT auto-preselect (no `onChange` with the default id — the Slice-4 fix);
// the Active entry is selected and maps to ""; the "Active — <label>" label reflects the workspace
// pick; every configured id is an explicit-override option; the schema hint the palette reads.

import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { hintFor } from "@/lib/channel/palette.types";
import type { InputSchema } from "@/lib/channel/palette.types";

// Mock the one API seam the widget reads: two configured runtimes, registry default = "default", and
// a workspace ACTIVE pick with a human label (the "Active — <label>" the composer renders).
vi.mock("@/lib/agent/runtimes.api", () => ({
  agentRuntimes: vi.fn(async () => ({
    default: "default",
    runtimes: ["acme-external", "default"],
    workspace_default: { runtime: "acme-external", label: "Acme Cloud" },
  })),
}));

import { RuntimeArg } from "./RuntimeArg";

describe("RuntimeArg widget", () => {
  it("does NOT auto-preselect on mount: no onChange with the default, Active entry maps to ''", async () => {
    const onChange = vi.fn();
    render(<RuntimeArg value="" onChange={onChange} />);

    // The dropdown loads: the Active entry (value "") plus each configured id as an override.
    const select = (await screen.findByLabelText("runtime")) as HTMLSelectElement;
    await waitFor(() =>
      expect(within(select).queryAllByRole("option").length).toBe(3),
    );
    const optionValues = within(select)
      .getAllByRole("option")
      .map((o) => (o as HTMLOptionElement).value);
    // The Active entry is first and maps to the EMPTY value → no runtime on the wire.
    expect(optionValues).toEqual(["", "acme-external", "default"]);

    // The picker defaults to the Active entry (empty value) and, crucially, the widget NEVER calls
    // onChange to pre-fill a runtime — an unset arg must MEAN "send no runtime" (the Slice-4 fix).
    expect(select.value).toBe("");
    expect(onChange).not.toHaveBeenCalled();

    // The Active entry reads the workspace pick's human label.
    expect(within(select).getByRole("option", { name: /active — acme cloud/i })).toBeInTheDocument();
  });

  it("reports a picked concrete override up on change (an explicit per-message runtime)", async () => {
    const onChange = vi.fn();
    render(<RuntimeArg value="" onChange={onChange} />);
    const user = userEvent.setup();

    const select = (await screen.findByLabelText("runtime")) as HTMLSelectElement;
    await waitFor(() =>
      expect(within(select).queryAllByRole("option").length).toBe(3),
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
