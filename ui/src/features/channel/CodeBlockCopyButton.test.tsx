// Unit tests for CodeBlockCopyButton (channels-agent scope) — the per-block "Copy code" affordance that
// sits in a fenced code/JSON block's header. The clipboard write is exercised via a stubbed
// `navigator.clipboard` (no gateway).

import { describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { CodeBlockCopyButton } from "./CodeBlockCopyButton";

describe("CodeBlockCopyButton", () => {
  it("writes the block source to the clipboard when clicked", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal("navigator", { clipboard: { writeText } });
    render(<CodeBlockCopyButton text="fn main() {}" />);
    await userEvent.click(screen.getByLabelText("copy code"));
    expect(writeText).toHaveBeenCalledWith("fn main() {}");
    vi.unstubAllGlobals();
    cleanup();
  });

  it("shows a brief Copied affordance after a successful copy, then reverts", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal("navigator", { clipboard: { writeText } });
    render(<CodeBlockCopyButton text="hello" />);
    await userEvent.click(screen.getByLabelText("copy code"));
    expect(screen.getByText("Copied")).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByText("Copy code")).toBeInTheDocument();
    }, { timeout: 2500 });
    vi.unstubAllGlobals();
    cleanup();
  });

  it("is disabled when the block source is empty", () => {
    render(<CodeBlockCopyButton text="" />);
    expect(screen.getByLabelText("copy code")).toBeDisabled();
    cleanup();
  });

  it("leaves the button unchanged when the clipboard denies the write", async () => {
    const writeText = vi.fn().mockRejectedValue(new Error("denied"));
    vi.stubGlobal("navigator", { clipboard: { writeText } });
    render(<CodeBlockCopyButton text="hello" />);
    await userEvent.click(screen.getByLabelText("copy code"));
    // No copied state — the Copy code affordance is still showing.
    expect(screen.getByText("Copy code")).toBeInTheDocument();
    expect(writeText).toHaveBeenCalledWith("hello");
    vi.unstubAllGlobals();
    cleanup();
  });
});
