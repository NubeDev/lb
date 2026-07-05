// The collapsed options drawer (data-studio-10x scope, phase 3 stage 3) — the query-first flow's
// "refine on demand": the full option surface folds behind one collapsed bar. Power depth intact,
// default cost zero. One responsibility: the disclosure chrome — the test asserts the collapsed/expand
// state and that the children (the OptionsSections surface incl. the search input) appear on expand.

import { describe, expect, it } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

import { OptionsDrawer } from "./OptionsDrawer";

describe("OptionsDrawer — the collapsed disclosure", () => {
  it("renders collapsed by default — the bar with the Options label, NO children yet", () => {
    render(
      <OptionsDrawer>
        <div data-testid="child">sections</div>
      </OptionsDrawer>,
    );
    const bar = screen.getByLabelText("options drawer");
    expect(bar).toHaveAttribute("aria-expanded", "false");
    // The children are absent in the collapsed state — zero cost by default.
    expect(screen.queryByTestId("child")).toBeNull();
  });

  it("expands on click — aria-expanded flips true and the children mount", () => {
    render(
      <OptionsDrawer>
        <div data-testid="child">sections</div>
      </OptionsDrawer>,
    );
    fireEvent.click(screen.getByLabelText("options drawer"));
    expect(screen.getByLabelText("options drawer")).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByTestId("child")).toBeInTheDocument();
  });

  it("collapses on a second click — aria-expanded flips back, children unmount", () => {
    render(
      <OptionsDrawer>
        <div data-testid="child">sections</div>
      </OptionsDrawer>,
    );
    const bar = screen.getByLabelText("options drawer");
    fireEvent.click(bar);
    expect(screen.getByTestId("child")).toBeInTheDocument();
    fireEvent.click(bar);
    expect(screen.queryByTestId("child")).toBeNull();
    expect(bar).toHaveAttribute("aria-expanded", "false");
  });

  it("the drawer's children carry the searchable OptionsSections (the search input mounts on expand)", () => {
    render(
      <OptionsDrawer>
        <input aria-label="search options" />
      </OptionsDrawer>,
    );
    // Collapsed: the search input is NOT in the DOM (the drawer folds the entire option surface).
    expect(screen.queryByLabelText("search options")).toBeNull();
    fireEvent.click(screen.getByLabelText("options drawer"));
    expect(screen.getByLabelText("search options")).toBeInTheDocument();
  });
});
