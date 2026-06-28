import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ThemeProvider } from "@/lib/theme";
import { ThemeSwitcher } from "./ThemeSwitcher";

afterEach(() => {
  cleanup();
  localStorage.clear();
  document.documentElement.className = "";
  delete document.documentElement.dataset.themeAccent;
  document.documentElement.removeAttribute("style");
});

describe("ThemeSwitcher", () => {
  it("switches mode and accent through accessible controls", async () => {
    const user = userEvent.setup();
    render(
      <ThemeProvider>
        <ThemeSwitcher />
      </ThemeProvider>,
    );

    expect(screen.getByLabelText("Use dark mode")).toHaveAttribute("aria-pressed", "true");
    expect(screen.getAllByLabelText("Use amber accent")[0]).toHaveAttribute("aria-pressed", "true");

    await user.click(screen.getByLabelText("Use light mode"));
    await user.click(screen.getAllByLabelText("Use blue accent")[0]);

    expect(document.documentElement).not.toHaveClass("dark");
    expect(document.documentElement.dataset.themeAccent).toBe("blue");
    expect(screen.getByLabelText("Use light mode")).toHaveAttribute("aria-pressed", "true");
    expect(screen.getAllByLabelText("Use blue accent")[0]).toHaveAttribute("aria-pressed", "true");
  });
});
