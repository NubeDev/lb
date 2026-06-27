// The Create-series wizard, driven against a REAL in-process gateway (data-console scope; CLAUDE §9 —
// no fake backend). Proves the whole feature end to end: the empty-state CTA opens the wizard, a typed
// schema (incl. a nested object) is built and persisted as a REAL record through the ingest path, the
// new series then appears in the explorer with a generated typed write form, and a typed sample writes
// and renders. A fresh workspace per test keeps the shared real node isolated.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { IngestView } from "./IngestView";
import { loadSchema } from "@/lib/ingest/schema.api";
import { useRealGateway, signInReal } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `wiz-${n++}`;

beforeAll(() => useRealGateway());

describe("CreateSeriesWizard (real gateway)", () => {
  it("empty workspace shows a create CTA instead of dead-ending", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    render(<IngestView ws={ws} />);
    // The first-run empty state offers a real action.
    expect(await screen.findByText(/no series yet/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /create series/i })).toBeInTheDocument();
  });

  it("creates a series with a nested schema; it persists and renders a typed form", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    render(<IngestView ws={ws} />);

    // Open the wizard from the empty-state CTA.
    await user.click(await screen.findByRole("button", { name: /create series/i }));

    // Step 1: name it.
    const dialog = await screen.findByRole("dialog", { name: /create series/i });
    await user.type(within(dialog).getByLabelText("series name"), "node.cpu_temp");
    await user.click(within(dialog).getByRole("button", { name: /next: schema/i }));

    // Step 2: the builder seeds one field — name it `celsius` (number, the default type).
    const nameInputs = within(dialog).getAllByLabelText(/^field name/i);
    await user.type(nameInputs[0], "celsius");

    // Add a nested OBJECT field `host` with a sub-field `rack`.
    await user.click(within(dialog).getByRole("button", { name: /add field/i }));
    const afterAdd = within(dialog).getAllByLabelText(/^field name/i);
    const hostInput = afterAdd[afterAdd.length - 1];
    await user.type(hostInput, "host");
    // Set that field's type to Object → reveals a nested builder.
    const typePickers = within(dialog).getAllByLabelText("field type");
    await user.selectOptions(typePickers[typePickers.length - 1], "object");
    // The nested "Add sub-field" appears; add `rack`.
    await user.click(within(dialog).getByRole("button", { name: /add sub-field/i }));
    const nested = within(dialog).getAllByLabelText(/^field name/i);
    await user.type(nested[nested.length - 1], "rack");

    // Create.
    await user.click(within(dialog).getByRole("button", { name: /create series/i }));

    // The schema persisted as a real record (read it back over the real gateway).
    await waitFor(async () => {
      const s = await loadSchema("node.cpu_temp");
      expect(s).not.toBeNull();
      expect(s!.fields.map((f) => f.name)).toEqual(["celsius", "host"]);
      expect(s!.fields.find((f) => f.name === "host")!.fields![0].name).toBe("rack");
    });

    // The new series is selected and a TYPED write form rendered (a labelled `celsius` input). The
    // form renders after the schema loads via a round-trip, so wait for the generated inputs.
    expect(await screen.findByLabelText("celsius")).toBeInTheDocument();
    // The nested object's field shows too.
    expect(await screen.findByLabelText("rack")).toBeInTheDocument();
  });

  it("writes a typed sample through the generated form", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    render(<IngestView ws={ws} />);

    // Create a minimal one-field series quickly.
    await user.click(await screen.findByRole("button", { name: /create series/i }));
    const dialog = await screen.findByRole("dialog", { name: /create series/i });
    await user.type(within(dialog).getByLabelText("series name"), "temp");
    await user.click(within(dialog).getByRole("button", { name: /next: schema/i }));
    await user.type(within(dialog).getAllByLabelText(/^field name/i)[0], "celsius");
    await user.click(within(dialog).getByRole("button", { name: /create series/i }));

    // Fill the generated typed form and push (wait for the schema-driven input to render).
    await user.type(await screen.findByLabelText("celsius"), "61");
    const form = screen.getByLabelText("write sample");
    await user.click(within(form).getByLabelText("submit sample"));

    // The latest value reflects the typed payload (a JSON object with celsius).
    const latest = await screen.findByLabelText("latest value");
    await waitFor(() => expect(latest).toHaveTextContent(/61/));
  });
});
