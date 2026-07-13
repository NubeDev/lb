// MarkdownEditor unit coverage (reports scope): the toolbar'd textarea editor's edit round-trip. The
// `value`/`onChange` contract is a MARKDOWN STRING both ways; the toolbar wraps/prefixes the selection;
// the Write/Preview toggle swaps the textarea for the rendered body; `editable={false}` renders ONLY
// the preview (the report sheet / print view reuse). jsdom has no real selection state, so the toolbar
// tests exercise the wrap/prefix mechanics against the whole string (selectionStart=selectionEnd=0 →
// caret) and assert onChange fires with the expected markdown.

import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { MarkdownEditor } from "./MarkdownEditor";

describe("MarkdownEditor — edit round-trip", () => {
  it("editing the textarea fires onChange with the full new markdown string", () => {
    // The editor is controlled (`value`/`onChange`), so simulate the user changing the whole value
    // via fireEvent.change — the contract is "onChange emits the current markdown string", and a
    // controlled textarea's onChange is the one wired event.
    const onChange = vi.fn();
    render(<MarkdownEditor value="" onChange={onChange} label="body" />);
    const ta = screen.getByLabelText("body") as HTMLTextAreaElement;
    fireEvent.change(ta, { target: { value: "hello world" } });
    expect(onChange).toHaveBeenLastCalledWith("hello world");
  });

  it("the Bold toolbar button wraps the selection in **", () => {
    const onChange = vi.fn();
    render(<MarkdownEditor value="word" onChange={onChange} label="body" />);
    const ta = screen.getByLabelText("body") as HTMLTextAreaElement;
    // Select "word" (the whole string), then click Bold.
    ta.setSelectionRange(0, 4);
    screen.getByRole("button", { name: "Bold" }).click();
    expect(onChange).toHaveBeenLastCalledWith("**word**");
  });

  it("the Heading toolbar button prefixes the line with ##", () => {
    const onChange = vi.fn();
    render(<MarkdownEditor value="title" onChange={onChange} label="body" />);
    const ta = screen.getByLabelText("body") as HTMLTextAreaElement;
    ta.setSelectionRange(0, 0); // caret at start of the line
    screen.getByRole("button", { name: "Heading" }).click();
    expect(onChange).toHaveBeenLastCalledWith("## title");
  });

  it("the Bullet-list toolbar button prefixes the line with - ", () => {
    const onChange = vi.fn();
    render(<MarkdownEditor value="item" onChange={onChange} label="body" />);
    const ta = screen.getByLabelText("body") as HTMLTextAreaElement;
    ta.setSelectionRange(0, 0);
    screen.getByRole("button", { name: "Bullet list" }).click();
    expect(onChange).toHaveBeenLastCalledWith("- item");
  });

  it("Preview mode renders the markdown body (not the textarea) and disables the toolbar", async () => {
    const user = userEvent.setup();
    render(<MarkdownEditor value={"# Rendered"} onChange={() => {}} label="body" />);
    // Write mode shows the textarea.
    expect(screen.getByLabelText("body")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /^Preview/ }));
    // Preview mode renders the heading; the textarea (textbox) is gone.
    expect(screen.getByText("Rendered").tagName).toBe("H1");
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    // Toolbar buttons are disabled in preview (can't wrap text you can't see).
    expect(screen.getByRole("button", { name: "Bold" })).toBeDisabled();
  });

  it("editable={false} renders ONLY the preview (the print view / report sheet)", () => {
    const { container } = render(
      <MarkdownEditor value="# Report title" onChange={() => {}} editable={false} label="report body" />,
    );
    // No toolbar, no textarea (textbox role) — just the rendered body.
    expect(screen.queryByRole("button", { name: "Bold" })).not.toBeInTheDocument();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    expect(container.querySelector("h1")).toBeInTheDocument();
    expect(screen.getByText("Report title").tagName).toBe("H1");
  });
});
