// Unit tests for AnswerCopyButton (channels-agent scope) — the per-message "copy the agent's answer"
// affordance. The clipboard write is exercised via a stubbed `navigator.clipboard` (no gateway).

import { describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { AnswerCopyButton } from "./AnswerCopyButton";

describe("AnswerCopyButton", () => {
  it("writes the answer text to the clipboard when clicked", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal("navigator", { clipboard: { writeText } });
    render(<AnswerCopyButton text="Use the workspace id in the record id prefix." />);
    await userEvent.click(screen.getByLabelText("copy agent answer"));
    expect(writeText).toHaveBeenCalledWith("Use the workspace id in the record id prefix.");
    vi.unstubAllGlobals();
    cleanup();
  });

  it("shows a brief Copied ✓ affordance after a successful copy, then reverts", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal("navigator", { clipboard: { writeText } });
    const { container } = render(<AnswerCopyButton text="hello" />);
    await userEvent.click(screen.getByLabelText("copy agent answer"));
    // The check icon replaces the clipboard icon while "copied".
    expect(container.querySelector(".lucide-check")).not.toBeNull();
    expect(container.querySelector(".lucide-clipboard-copy")).toBeNull();
    // After the brief timeout, the copy affordance reverts.
    await waitFor(
      () => {
        expect(container.querySelector(".lucide-clipboard-copy")).not.toBeNull();
        expect(container.querySelector(".lucide-check")).toBeNull();
      },
      { timeout: 2500 },
    );
    vi.unstubAllGlobals();
    cleanup();
  });

  it("is disabled (no copy) when the answer is empty", () => {
    render(<AnswerCopyButton text="" />);
    expect(screen.getByLabelText("copy agent answer")).toBeDisabled();
    cleanup();
  });

  it("leaves the button unchanged when the clipboard denies the write", async () => {
    const writeText = vi.fn().mockRejectedValue(new Error("denied"));
    vi.stubGlobal("navigator", { clipboard: { writeText } });
    const { container } = render(<AnswerCopyButton text="hello" />);
    await userEvent.click(screen.getByLabelText("copy agent answer"));
    // No copied state — the copy affordance is still showing.
    expect(container.querySelector(".lucide-clipboard-copy")).not.toBeNull();
    expect(container.querySelector(".lucide-check")).toBeNull();
    expect(writeText).toHaveBeenCalledWith("hello");
    vi.unstubAllGlobals();
    cleanup();
  });
});
