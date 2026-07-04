import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ThemeProvider, useTheme } from "@/lib/theme";
import { LayoutTab } from "./LayoutTab";

// A probe that surfaces the resolved layout so we can assert LayoutTab drives `theme.layout` (which
// NavRail spreads onto the shipped shadcn <Sidebar variant/collapsible/side>).
function LayoutProbe() {
  const { theme } = useTheme();
  const { variant, collapsible, side } = theme.layout;
  return <output aria-label="layout">{`${variant}:${collapsible}:${side}`}</output>;
}

afterEach(() => {
  cleanup();
  localStorage.clear();
});

describe("LayoutTab", () => {
  it("switches sidebar variant, collapsible mode, and position through accessible cards", async () => {
    const user = userEvent.setup();
    render(
      <ThemeProvider>
        <LayoutTab />
        <LayoutProbe />
      </ThemeProvider>,
    );

    // Default layout (from DEFAULT_THEME).
    expect(screen.getByLabelText("layout")).toHaveTextContent("sidebar:icon:left");
    expect(screen.getByLabelText("Sidebar variant Default")).toHaveAttribute("aria-pressed", "true");

    await user.click(screen.getByLabelText("Sidebar variant Floating"));
    await user.click(screen.getByLabelText("Collapsible mode Off Canvas"));
    await user.click(screen.getByLabelText("Sidebar position Right"));

    expect(screen.getByLabelText("layout")).toHaveTextContent("floating:offcanvas:right");
    expect(screen.getByLabelText("Sidebar variant Floating")).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByLabelText("Sidebar position Right")).toHaveAttribute("aria-pressed", "true");
  });
});
