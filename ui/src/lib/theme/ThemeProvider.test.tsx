import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ThemeProvider } from "./ThemeProvider";
import { DEFAULT_LAYOUT, THEME_STORAGE_KEY } from "./theme-options";
import { useTheme } from "./useTheme";

// Note: in the jsdom `pnpm test` env there is no Tauri and no gateway, so the provider's mount-time
// `prefs.resolve` reconcile and its debounced `prefs.set` persist both throw inside `invoke` and are
// caught — the provider degrades to localStorage-only, which is exactly the "denied / offline" path.
// The prefs round-trip itself is proven against a REAL gateway in the *.gateway.test.tsx suite.

function ThemeProbe() {
  const { theme, setMode, setPreset } = useTheme();

  return (
    <div>
      <output aria-label="theme">{`${theme.mode}:${theme.preset}`}</output>
      <button type="button" onClick={() => setMode("light")}>
        light
      </button>
      <button type="button" onClick={() => setPreset("blue")}>
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
  it("loads from the cache, applies, and persists to the cache on change", async () => {
    const user = userEvent.setup();
    localStorage.setItem(
      THEME_STORAGE_KEY,
      JSON.stringify({ mode: "dark", preset: "teal", radius: "0.5rem" }),
    );

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

    // The localStorage cache reflects the change (prefs is best-effort and unavailable in jsdom).
    await waitFor(() =>
      expect(JSON.parse(localStorage.getItem(THEME_STORAGE_KEY) ?? "{}")).toEqual({
        mode: "light",
        preset: "blue",
        radius: "0.5rem",
        look: "default",
        layout: DEFAULT_LAYOUT,
      }),
    );
  });
});
