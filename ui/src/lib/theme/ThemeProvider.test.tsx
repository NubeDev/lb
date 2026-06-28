import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ThemeProvider } from "./ThemeProvider";
import { THEME_STORAGE_KEY } from "./theme-options";
import { useTheme } from "./useTheme";

function ThemeProbe() {
  const { theme, setMode, setAccent } = useTheme();

  return (
    <div>
      <output aria-label="theme">{`${theme.mode}:${theme.accent}`}</output>
      <button type="button" onClick={() => setMode("light")}>
        light
      </button>
      <button type="button" onClick={() => setAccent("blue")}>
        blue
      </button>
    </div>
  );
}

afterEach(() => {
  cleanup();
  localStorage.clear();
  document.documentElement.className = "";
  delete document.documentElement.dataset.themeAccent;
  document.documentElement.removeAttribute("style");
});

describe("ThemeProvider", () => {
  it("loads, applies, and persists the theme preference", async () => {
    const user = userEvent.setup();
    localStorage.setItem(THEME_STORAGE_KEY, JSON.stringify({ mode: "dark", accent: "teal" }));

    render(
      <ThemeProvider>
        <ThemeProbe />
      </ThemeProvider>,
    );

    expect(screen.getByLabelText("theme")).toHaveTextContent("dark:teal");
    expect(document.documentElement).toHaveClass("dark");
    expect(document.documentElement.dataset.themeAccent).toBe("teal");

    await user.click(screen.getByRole("button", { name: "light" }));
    await user.click(screen.getByRole("button", { name: "blue" }));

    expect(screen.getByLabelText("theme")).toHaveTextContent("light:blue");
    expect(document.documentElement).not.toHaveClass("dark");
    expect(document.documentElement.dataset.themeAccent).toBe("blue");
    expect(JSON.parse(localStorage.getItem(THEME_STORAGE_KEY) ?? "{}")).toEqual({
      mode: "light",
      accent: "blue",
    });
  });
});
