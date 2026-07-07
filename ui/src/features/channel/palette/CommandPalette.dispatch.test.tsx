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

// `status` is REQUIRED here so the rail activates it and the "collected args merge into source.args" /
// "verbatim args" proofs have an arg to collect. (An OPTIONAL arg is skippable and does not block submit —
// covered by LIST_NOARG_CMD below, which reproduces the `/reminders`-with-only-optional-filters case.)
const STATUS_SCHEMA = {
  type: "object",
  properties: { status: { type: "string", "x-lb": { widget: "text" as const } } },
  required: ["status"],
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
// A result-declaring command whose only args are OPTIONAL filters (like `reminder.list`'s status/limit) —
// it must be runnable the INSTANT it is picked, with NO arg text box demanding input (the `/reminders` UX
// fix). Accepting the command and pressing send posts its render with no collected filter args.
const LIST_NOARG_CMD: ToolDescriptor = {
  name: "things.listall",
  title: "things.listall",
  group: "things",
  input_schema: {
    type: "object",
    properties: { status: { type: "string", "x-lb": { widget: "text" as const } } },
    // no `required` — every arg is an optional filter
  },
  result: {
    view: "table",
    source: { tool: "things.listall", args: {} },
    tools: ["things.listall"],
  },
};

// A GENERIC descriptor with CONDITIONALLY-REQUIRED fields (the `/remind` shape, tool-agnostic): a
// `select` `kind` gates two per-kind fields via `x-lb.showIf`. `channel` is required when shown
// (`requiredWhenShown`), `note` is shown-but-optional. This is the exact mechanism that makes the
// reminder action fields reachable — the palette carries ZERO knowledge that it is a reminder.
const FORM_CMD: ToolDescriptor = {
  name: "things.form",
  title: "things.form",
  group: "things",
  input_schema: {
    type: "object",
    properties: {
      kind: { type: "string", "x-lb": { widget: "select" as const, options: ["a", "b"] } },
      channel: { type: "string", "x-lb": { showIf: { kind: "a" }, requiredWhenShown: true } },
      note: { type: "string", "x-lb": { showIf: { kind: "a" } } },
    },
    required: ["kind"],
  },
};

// A required chip arg (`goal`) plus an OPTIONAL INLINE widget (`pick`, a `select`) — the agent
// command's shape (required `goal` + optional `runtime` picker), tool-agnostic. Regression guard: an
// optional inline widget was previously UNREACHABLE (the rail only walked REQUIRED args), so the runtime
// dropdown never rendered — the "no option to pick a runtime" bug. The optional-inline rail tier must
// surface it once `goal` is filled AND keep it mounted after it preselects (so the choice stays editable).
const OPT_INLINE_CMD: ToolDescriptor = {
  name: "things.pick",
  title: "things.pick",
  group: "things",
  input_schema: {
    type: "object",
    properties: {
      goal: { type: "string" },
      pick: { type: "string", "x-lb": { widget: "select" as const, options: ["one", "two"] } },
    },
    required: ["goal"],
  },
};

// A REQUIRED INLINE widget (`q`, a `sql` editor) — the `/query` shape without the entity arg.
// Regression guard (query re-edit scope): a required inline arg used to cycle through the single
// active slot, so ONE typed character marked it "filled" and UNMOUNTED the editor mid-typing (the
// "sql editor closes on the first keystroke" bug). It must render persistently and stay mounted.
const SQL_CMD: ToolDescriptor = {
  name: "things.query",
  title: "things.query",
  group: "things",
  input_schema: {
    type: "object",
    properties: { q: { type: "string", "x-lb": { widget: "sql" as const } } },
    required: ["q"],
  },
};

// Stub the two data hooks so the palette renders from a fixed catalog with no network (a thin stub, not a
// node re-implementation — rule 9). `useMentions` is unused by these fixtures (no entity args).
vi.mock("./useCatalog", () => ({
  useCatalog: () => ({
    tools: [LIST_CMD, CREATE_CMD, LIST_NOARG_CMD, FORM_CMD, OPT_INLINE_CMD, SQL_CMD],
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

  // The `/reminders` UX fix: a command whose only args are OPTIONAL filters is runnable the instant it is
  // picked — NO arg text box blocks it. Accept the command, press send immediately, the render is posted.
  it("runs a command with only OPTIONAL args immediately — no arg box, no blocked submit", async () => {
    const onPostRich = vi.fn();
    const user = userEvent.setup();
    render(
      <CommandPalette
        channel="general"
        onPostQuery={noop}
        onSendAgent={noop}
        onCallTool={noop}
        onPostRich={onPostRich}
        onSendChat={noop}
      />,
    );
    await user.type(screen.getByLabelText("message"), "/things.listall");
    await screen.findByRole("listbox", { name: "commands" });
    await user.keyboard("{Enter}");

    // No arg widget demands input — the optional `status` filter never activates.
    expect(screen.queryByLabelText("status")).toBeNull();

    // Send is immediately enabled and posts the declared render with no collected filter args.
    await user.click(screen.getByLabelText("send"));
    expect(onPostRich).toHaveBeenCalledTimes(1);
    expect(onPostRich).toHaveBeenCalledWith(
      encodeRichResult({
        view: "table",
        source: { tool: "things.listall", args: {} },
        tools: ["things.listall"],
      }),
    );
  });

  // The `/remind` fix (tool-agnostic): a CONDITIONALLY-required field (`x-lb.showIf` +
  // `requiredWhenShown`) enters the active-arg walk once its condition matches, blocks submit until
  // filled, and is then sent — the mechanism that made the reminder action fields reachable.
  it("surfaces a conditionally-required field once its showIf matches, blocks submit, then sends it", async () => {
    const onCallTool = vi.fn();
    const user = userEvent.setup();
    render(
      <CommandPalette
        channel="general"
        onPostQuery={noop}
        onSendAgent={noop}
        onCallTool={onCallTool}
        onPostRich={noop}
        onSendChat={noop}
      />,
    );
    await user.type(screen.getByLabelText("message"), "/things.form");
    await screen.findByRole("listbox", { name: "commands" });
    await user.keyboard("{Enter}");

    // `kind` (select) preselects its first option "a" → the `showIf:{kind:"a"}` fields activate. The
    // required `channel` now demands input and BLOCKS submit (it was unreachable before this fix).
    const channel = await screen.findByLabelText("channel");
    expect(screen.getByLabelText("send")).toBeDisabled();

    // Filling `channel` satisfies the last active-required field → submit enables and sends all fields
    // (kind + the shown channel), VERBATIM through the bridge (no result declared).
    await user.type(channel, "standup");
    const send = screen.getByLabelText("send");
    expect(send).toBeEnabled();
    await user.click(send);
    expect(onCallTool).toHaveBeenCalledWith("things.form", { kind: "a", channel: "standup" });
  });

  // Regression (the "no runtime picker" bug): an OPTIONAL INLINE widget must surface once the required
  // arg is filled, and STAY mounted after it self-selects a default — so the user can still change it.
  // Before the optional-inline rail tier the widget never rendered at all.
  it("renders an optional inline widget PERSISTENTLY beside the required arg (not gated behind ⏎), and sends the choice", async () => {
    const onCallTool = vi.fn();
    const user = userEvent.setup();
    render(
      <CommandPalette
        channel="general"
        onPostQuery={noop}
        onSendAgent={noop}
        onCallTool={onCallTool}
        onPostRich={noop}
        onSendChat={noop}
      />,
    );
    await user.type(screen.getByLabelText("message"), "/things.pick");
    await screen.findByRole("listbox", { name: "commands" });
    await user.keyboard("{Enter}");

    // THE FIX (the "no runtime picker" bug): the required `goal` field AND the optional inline `select`
    // are BOTH visible the instant the command is picked — the optional inline widget renders
    // persistently, NOT as the "next active arg" gated behind committing `goal` with a hidden ⏎.
    const goal = await screen.findByLabelText("goal");
    const pick = (await screen.findByLabelText("select")) as HTMLSelectElement;

    // Type the goal WITHOUT pressing Enter (how a user actually fills it) — the picker stays shown.
    await user.type(goal, "ship it");
    expect(screen.getByLabelText("select")).toBeInTheDocument();

    // Change the choice and send — both the typed goal (folded from the in-progress field) and the
    // picked runtime ride the call.
    await user.selectOptions(pick, "two");
    await user.click(screen.getByLabelText("send"));
    expect(onCallTool).toHaveBeenCalledWith("things.pick", { goal: "ship it", pick: "two" });
  });

  // Regression (query re-edit scope): a REQUIRED inline widget (the sql editor) must render
  // persistently and STAY MOUNTED while the user types. Before the fix, the first keystroke counted
  // the arg "filled", the single active slot moved on, and the editor UNMOUNTED mid-typing — the
  // "sql editor closes and can't be reopened" bug.
  it("keeps a REQUIRED inline sql editor mounted while typing, blocks empty submit, then sends", async () => {
    const onCallTool = vi.fn();
    const user = userEvent.setup();
    render(
      <CommandPalette
        channel="general"
        onPostQuery={noop}
        onSendAgent={noop}
        onCallTool={onCallTool}
        onPostRich={noop}
        onSendChat={noop}
      />,
    );
    await user.type(screen.getByLabelText("message"), "/things.query");
    await screen.findByRole("listbox", { name: "commands" });
    await user.keyboard("{Enter}");

    // Empty required sql → send is disabled (not runnable yet), but the editor is mounted.
    const sql = await screen.findByLabelText("sql");
    expect(screen.getByLabelText("send")).toBeDisabled();

    // Type a whole statement — the editor survives every keystroke (THE regression).
    await user.type(sql, "SELECT 1 FROM t");
    expect(screen.getByLabelText("sql")).toBeInTheDocument();
    expect((screen.getByLabelText("sql") as HTMLTextAreaElement).value).toBe("SELECT 1 FROM t");

    await user.click(screen.getByLabelText("send"));
    expect(onCallTool).toHaveBeenCalledWith("things.query", { q: "SELECT 1 FROM t" });
  });

  // PREFILL (query re-edit scope): "edit this query" reopens the palette with the tool accepted and
  // its args seeded — an inline arg (sql) seeds its live widget — so the user never re-walks the
  // selection stage. One-shot: the parent's consume callback fires.
  it("prefill opens the tool with args seeded into the inline widget and is consumed once", async () => {
    const onCallTool = vi.fn();
    const onPrefillConsumed = vi.fn();
    const user = userEvent.setup();
    render(
      <CommandPalette
        channel="general"
        onPostQuery={noop}
        onSendAgent={noop}
        onCallTool={onCallTool}
        onPostRich={noop}
        onSendChat={noop}
        prefill={{ tool: "things.query", args: { q: "SELECT 2" } }}
        onPrefillConsumed={onPrefillConsumed}
      />,
    );
    // The tool opens pre-accepted with the sql editor seeded — no command menu walk.
    const sql = (await screen.findByLabelText("sql")) as HTMLTextAreaElement;
    expect(sql.value).toBe("SELECT 2");
    expect(onPrefillConsumed).toHaveBeenCalled();

    // The seeded value is editable and sends verbatim.
    await user.type(sql, " -- edited");
    await user.click(screen.getByLabelText("send"));
    expect(onCallTool).toHaveBeenCalledWith("things.query", { q: "SELECT 2 -- edited" });
  });
});
