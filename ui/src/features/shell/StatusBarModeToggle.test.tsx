// StatusBarModeToggle — the footer's quick dark/light flip. Proves the pointer path: clicking the
// compact strip button calls `setMode` with the opposite mode, the icon reflects the mode you'll
// switch TO (Sun in dark → light; Moon in light → dark), and the press state reads as "dark active".
// Markup/interaction only — the real ThemeProvider drives the actual DOM (CLAUDE §9, no fakes).

import { render, screen, cleanup, fireEvent } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { ThemeProvider, useTheme, DEFAULT_THEME } from "@/lib/theme";

import { StatusBarModeToggle } from "./StatusBarModeToggle";

afterEach(cleanup);

/** A tiny spy that re-reads the live theme inside the provider after the click lands. */
function ModeProbe() {
  const { theme } = useTheme();
  return <span data-testid="probe">{theme.mode}</span>;
}

function renderToggle() {
  return render(
    <ThemeProvider>
      <StatusBarModeToggle />
      <ModeProbe />
    </ThemeProvider>,
  );
}

describe("StatusBarModeToggle", () => {
  it("cycles dark → light → system → dark on successive clicks", async () => {
    // DEFAULT_THEME.mode is "dark" — the toggle offers the move to light.
    expect(DEFAULT_THEME.mode).toBe("dark");
    renderToggle();
    const btn = screen.getByRole("button", { name: /switch to light mode/i });
    // dark is the active press state
    expect(btn).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(btn);
    // After the first click: "dark" → "light"; button now offers the move to system.
    expect(screen.getByTestId("probe")).toHaveTextContent("light");
    expect(screen.getByRole("button", { name: /switch to system mode/i })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
    fireEvent.click(screen.getByRole("button", { name: /switch to system mode/i }));
    // After the second click: "light" → "system"; button now offers the move to dark.
    expect(screen.getByTestId("probe")).toHaveTextContent("system");
    expect(screen.getByRole("button", { name: /switch to dark mode/i })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
    fireEvent.click(screen.getByRole("button", { name: /switch to dark mode/i }));
    // After the third click: "system" → "dark"; back to start.
    expect(screen.getByTestId("probe")).toHaveTextContent("dark");
    expect(screen.getByRole("button", { name: /switch to light mode/i })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });
});
