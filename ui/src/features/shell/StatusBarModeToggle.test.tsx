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
  it("renders a Sun button in dark mode that switches to light on click", async () => {
    // DEFAULT_THEME.mode is "dark" — the toggle offers the opposite move (to light).
    expect(DEFAULT_THEME.mode).toBe("dark");
    renderToggle();
    const btn = screen.getByRole("button", { name: /switch to light mode/i });
    // dark is the active press state
    expect(btn).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(btn);
    // The provider reflects the new mode, and the button now offers the move back to dark.
    expect(screen.getByTestId("probe")).toHaveTextContent("light");
    expect(screen.getByRole("button", { name: /switch to dark mode/i })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
  });
});
