// Unit test for the palette's GENERIC submit rule (channel rich responses scope). NO gateway — the
// catalog + mention hooks are stubbed so this is a pure controller test. The point it proves is the
// whole design: the palette is TOOL-AGNOSTIC. A descriptor that DECLARES a `result` render-envelope has
// that render POSTED (with the collected args interpolated into `source.args`) — with ZERO knowledge of
// which tool it is; a descriptor WITHOUT `result` posts its collected form fields VERBATIM via the plain
// bridge call. The fixtures below are GENERIC commands (`things.*`), not reminders: any descriptor.result
// flows through the same path.

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { ToolDescriptor } from "@/lib/channel/palette.types";
import { encodeRichResult } from "@/lib/channel/payload.types";

const STATUS_SCHEMA = {
  type: "object",
  properties: { status: { type: "string", "x-lb": { widget: "text" as const } } },
};

// A GENERIC descriptor that DECLARES a `result` render (a source-backed table with a row control) — NOT a
// reminder. And a GENERIC descriptor WITHOUT a result (the plain-call path).
const LIST_CMD: ToolDescriptor = {
  name: "things.list",
  title: "things.list",
  group: "things",
  input_schema: STATUS_SCHEMA,
  result: {
    view: "table",
    source: { tool: "things.list", args: { limit: 20 } },
    options: { rowControls: [{ kind: "button", buttonLabel: "Del", action: { tool: "things.delete", argsTemplate: { id: "${id}" } } }] },
    tools: ["things.list", "things.delete"],
  },
};
const CREATE_CMD: ToolDescriptor = {
  name: "things.create",
  title: "things.create",
  group: "things",
  input_schema: STATUS_SCHEMA,
};

// Stub the two data hooks so the palette renders from a fixed catalog with no network (a thin stub, not a
// node re-implementation — rule 9). `useMentions` is unused by these fixtures (no entity args).
vi.mock("./useCatalog", () => ({
  useCatalog: () => ({
    tools: [LIST_CMD, CREATE_CMD],
    loading: false,
    error: null,
    revalidate: async () => {},
  }),
}));
vi.mock("./useMentions", () => ({
  useMentions: () => ({ items: [], loading: false, reason: null }),
}));

import { CommandPalette } from "./CommandPalette";

function noop() {}

/** Render the palette, accept `/<command>`, fill the `status` arg with `value`, and press send. */
async function runCommand(
  command: string,
  value: string,
  handlers: { onPostRich: ReturnType<typeof vi.fn>; onCallTool: ReturnType<typeof vi.fn> },
) {
  const user = userEvent.setup();
  render(
    <CommandPalette
      channel="general"
      onPostQuery={noop}
      onSendAgent={noop}
      onCallTool={handlers.onCallTool}
      onPostRich={handlers.onPostRich}
      onSendChat={noop}
    />,
  );
  await user.type(screen.getByLabelText("message"), `/${command}`);
  await screen.findByRole("listbox", { name: "commands" });
  await user.keyboard("{Enter}");
  const arg = await screen.findByLabelText("status");
  await user.type(arg, value);
  await user.click(screen.getByLabelText("send"));
}

describe("CommandPalette — the generic descriptor.result submit rule", () => {
  it("posts the descriptor's declared render with the collected args merged into source.args", async () => {
    const onPostRich = vi.fn();
    const onCallTool = vi.fn();
    await runCommand("things.list", "open", { onPostRich, onCallTool });

    // The GENERIC path fired: a rich_result was POSTED (never a plain onCallTool for a result descriptor).
    expect(onPostRich).toHaveBeenCalledTimes(1);
    expect(onCallTool).not.toHaveBeenCalled();

    // The posted body = the descriptor's render, with the collected `status` merged into source.args over
    // the descriptor's own args. NO tool-name branch produced this — it is the descriptor's declared shape.
    const expected = encodeRichResult({
      view: "table",
      source: { tool: "things.list", args: { limit: 20, status: "open" } },
      options: LIST_CMD.result!.options,
      tools: LIST_CMD.result!.tools,
    });
    expect(onPostRich).toHaveBeenCalledWith(expected);
  });

  it("posts a PLAIN bridge call (verbatim args) when the descriptor declares no result", async () => {
    const onPostRich = vi.fn();
    const onCallTool = vi.fn();
    await runCommand("things.create", "open", { onPostRich, onCallTool });

    // The else-branch is generic too: the collected form fields go verbatim — no reshaping, no confirmation.
    expect(onPostRich).not.toHaveBeenCalled();
    expect(onCallTool).toHaveBeenCalledWith("things.create", { status: "open" });
  });
});
