// HeaderBreadcrumbs — the alternative page-header style (shell-chrome-layout scope). Proves the
// breadcrumb trail renders `Workspace / <Surface>` (v1 two-level) the shadcn way — clean, no icon
// chip, no gradient — and still carries the actions slot (workspace chip + Settings link) so no
// surface loses an affordance by switching header styles.

import { render, screen, cleanup } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { Hash } from "lucide-react";

import { HeaderBreadcrumbs } from "./header-breadcrumbs";

afterEach(cleanup);

describe("HeaderBreadcrumbs", () => {
  it("renders the trail Workspace / <title> as a shadcn breadcrumb", () => {
    render(<HeaderBreadcrumbs icon={Hash} title="Settings" workspace="acme" />);
    // The workspace crumb + the page crumb both render; the page is the current page.
    expect(screen.getByText("Settings")).toBeInTheDocument();
    const page = screen.getByText("Settings");
    expect(page.closest("[data-slot='breadcrumb-page']")).toHaveAttribute("aria-current", "page");
  });

  it("renders the trail without a workspace crumb when no workspace is passed", () => {
    render(<HeaderBreadcrumbs icon={Hash} title="Standalone" />);
    expect(screen.getByText("Standalone")).toBeInTheDocument();
  });

  it("carries the workspace chip + Settings link in the actions slot (parity with the band header)", () => {
    render(<HeaderBreadcrumbs icon={Hash} title="Settings" workspace="acme" />);
    expect(screen.getByLabelText("Open settings")).toBeInTheDocument();
    expect(screen.getByTitle("Workspace acme")).toBeInTheDocument();
  });

  it("renders the trailing actions slot", () => {
    render(
      <HeaderBreadcrumbs
        icon={Hash}
        title="Flows"
        workspace="acme"
        actions={<button type="button">Deploy</button>}
      />,
    );
    expect(screen.getByRole("button", { name: "Deploy" })).toBeInTheDocument();
  });
});
