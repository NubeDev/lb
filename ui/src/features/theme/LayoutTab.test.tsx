import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ThemeProvider, useTheme } from "@/lib/theme";
import { LayoutTab } from "./LayoutTab";

// A probe that surfaces the resolved layout so we can assert LayoutTab drives `theme.layout` (which
// NavRail spreads onto the shipped shadcn <Sidebar variant/collapsible/side> and which AppPage/
// RoutedShell read for header style + nav mode).
function LayoutProbe() {
  const { theme } = useTheme();
  const { variant, collapsible, side, header, nav } = theme.layout;
  return <output aria-label="layout">{`${variant}:${collapsible}:${side}:${header}:${nav}`}</output>;
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
    expect(screen.getByLabelText("layout")).toHaveTextContent("sidebar:icon:left:band:sidebar");
    expect(screen.getByLabelText("Sidebar variant Default")).toHaveAttribute("aria-pressed", "true");

    await user.click(screen.getByLabelText("Sidebar variant Floating"));
    await user.click(screen.getByLabelText("Collapsible mode Off Canvas"));
    await user.click(screen.getByLabelText("Sidebar position Right"));

    expect(screen.getByLabelText("layout")).toHaveTextContent("floating:offcanvas:right:band:sidebar");
    expect(screen.getByLabelText("Sidebar variant Floating")).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByLabelText("Sidebar position Right")).toHaveAttribute("aria-pressed", "true");
  });

  it("switches header style and navigation mode through the new option cards", async () => {
    const user = userEvent.setup();
    render(
      <ThemeProvider>
        <LayoutTab />
        <LayoutProbe />
      </ThemeProvider>,
    );

    expect(screen.getByLabelText("layout")).toHaveTextContent("sidebar:icon:left:band:sidebar");

    await user.click(screen.getByLabelText("Header style Breadcrumbs"));
    await user.click(screen.getByLabelText("Navigation Top menu"));

    expect(screen.getByLabelText("layout")).toHaveTextContent("sidebar:icon:left:breadcrumbs:topmenu");
    expect(screen.getByLabelText("Header style Breadcrumbs")).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByLabelText("Navigation Top menu")).toHaveAttribute("aria-pressed", "true");
  });

  it("marks the sidebar axes 'sidebar only' and keeps their values when nav is topmenu", async () => {
    const user = userEvent.setup();
    render(
      <ThemeProvider>
        <LayoutTab />
      </ThemeProvider>,
    );

    // Pick a non-default sidebar axis value, then switch nav to topmenu.
    await user.click(screen.getByLabelText("Sidebar variant Floating"));
    await user.click(screen.getByLabelText("Navigation Top menu"));

    // The "sidebar only" hint appears on each sidebar-axis hint text.
    const hints = screen.getAllByText(/sidebar only/);
    expect(hints.length).toBe(3); // variant + collapsible + position
    // The Floating card is still selected (the value was NOT cleared).
    expect(screen.getByLabelText("Sidebar variant Floating")).toHaveAttribute("aria-pressed", "true");

    // Switching back to sidebar keeps the value intact (no hidden state lost).
    await user.click(screen.getByLabelText("Navigation Sidebar"));
    expect(screen.getByLabelText("Sidebar variant Floating")).toHaveAttribute("aria-pressed", "true");
    expect(screen.queryByText(/sidebar only/)).toBeNull();
  });
});
