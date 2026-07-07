// The options drawer (data-studio-10x scope, phase 3 stage 3) — the query-first flow's option
// surface behind one disclosure bar. OPEN by default: editing the chart must never be hidden (the
// collapse exists to reclaim preview space, not as the resting state — the live finding that flipped
// it). One responsibility: the disclosure chrome — the test asserts the open default, the collapse/
// re-expand toggle, and that the children (the OptionsSections surface incl. the search input)
// unmount only while collapsed.

import { describe, expect, it } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

import { OptionsDrawer } from "./OptionsDrawer";

describe("OptionsDrawer — the disclosure", () => {
  it("renders OPEN by default — the option surface is never hidden at rest", () => {
    render(
      <OptionsDrawer>
        <div data-testid="child">sections</div>
      </OptionsDrawer>,
    );
    const bar = screen.getByLabelText("options drawer");
    expect(bar).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByTestId("child")).toBeInTheDocument();
  });

  it("collapses on click — aria-expanded flips false and the children unmount", () => {
    render(
      <OptionsDrawer>
        <div data-testid="child">sections</div>
      </OptionsDrawer>,
    );
    fireEvent.click(screen.getByLabelText("options drawer"));
    expect(screen.getByLabelText("options drawer")).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByTestId("child")).toBeNull();
  });

  it("re-expands on a second click — aria-expanded flips back, children remount", () => {
    render(
      <OptionsDrawer>
        <div data-testid="child">sections</div>
      </OptionsDrawer>,
    );
    const bar = screen.getByLabelText("options drawer");
    fireEvent.click(bar);
    expect(screen.queryByTestId("child")).toBeNull();
    fireEvent.click(bar);
    expect(screen.getByTestId("child")).toBeInTheDocument();
    expect(bar).toHaveAttribute("aria-expanded", "true");
  });

  it("the drawer's children carry the searchable OptionsSections (the search input is live at rest)", () => {
    render(
      <OptionsDrawer>
        <input aria-label="search options" />
      </OptionsDrawer>,
    );
    // Open at rest: the search input is in the DOM; collapsing folds the entire option surface.
    expect(screen.getByLabelText("search options")).toBeInTheDocument();
    fireEvent.click(screen.getByLabelText("options drawer"));
    expect(screen.queryByLabelText("search options")).toBeNull();
  });
});
