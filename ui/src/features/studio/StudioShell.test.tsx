// StudioShell renders the merged Studio page's tab bar from the caps the caller allows, and reports
// tab selection via `onSelectTab` (the route folds that into a URL change so tabs are deep-linkable).
// Each tab is a distinct CoreSurface with its own cap; the shell shows only the allowed ones. Markup +
// selection wiring only — the route owns navigation, the gateway owns the boundary.

import { render, screen, cleanup, fireEvent } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { StudioShell, type StudioTab } from "./StudioShell";
import { ThemeProvider } from "@/lib/theme/ThemeProvider";

afterEach(cleanup);

function renderShell(props: {
  tab: StudioTab;
  allowedTabs: StudioTab[];
  onSelectTab?: (t: StudioTab) => void;
}) {
  return render(
    <ThemeProvider>
      <StudioShell
        ws="acme"
        tab={props.tab}
        allowedTabs={props.allowedTabs}
        onSelectTab={props.onSelectTab ?? vi.fn()}
      >
        <div>body:{props.tab}</div>
      </StudioShell>
    </ThemeProvider>,
  );
}

describe("StudioShell", () => {
  it("renders only the allowed tabs", () => {
    renderShell({ tab: "extensions", allowedTabs: ["extensions", "build"] });
    expect(screen.getByRole("tab", { name: "Extensions" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Build" })).toBeInTheDocument();
  });

  it("hides a tab whose cap the session lacks (extensions-only session shows no Build tab)", () => {
    renderShell({ tab: "extensions", allowedTabs: ["extensions"] });
    expect(screen.getByRole("tab", { name: "Extensions" })).toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "Build" })).not.toBeInTheDocument();
  });

  it("reports tab selection via onSelectTab (the route turns it into a deep-linkable URL)", () => {
    const onSelectTab = vi.fn();
    renderShell({ tab: "extensions", allowedTabs: ["extensions", "build"], onSelectTab });
    fireEvent.click(screen.getByRole("tab", { name: "Build" }));
    expect(onSelectTab).toHaveBeenCalledWith("build");
  });

  it("renders the active tab's body", () => {
    renderShell({ tab: "build", allowedTabs: ["extensions", "build"] });
    expect(screen.getByText("body:build")).toBeInTheDocument();
  });
});
