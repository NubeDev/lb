// Real-component tests for NavMenu (the embeddable rail) — real items, real DOM/interaction.

import { fireEvent, render, screen } from "@testing-library/react";
import { Boxes } from "lucide-react";
import { describe, expect, it, vi } from "vitest";

import { NavMenu } from "./NavMenu";
import type { NavItem } from "./items";

const ITEMS: NavItem[] = [
  { id: "query", label: "Query", icon: Boxes },
  { id: "transform", label: "Transform", group: "Data" },
  { id: "overrides", label: "Overrides", group: "Data" },
];

describe("NavMenu", () => {
  it("renders items in order, grouped by `group`", () => {
    render(<NavMenu items={ITEMS} active={null} onSelect={() => {}} />);
    expect(screen.getByText("Data")).toBeTruthy(); // group label
    const labels = screen.getAllByRole("tab").map((b) => b.getAttribute("aria-label"));
    expect(labels).toEqual(["Query", "Transform", "Overrides"]);
  });

  it("calls onSelect and marks the active item", () => {
    const onSelect = vi.fn();
    render(<NavMenu items={ITEMS} active="query" onSelect={onSelect} />);
    expect(screen.getByRole("tab", { name: "Query" }).getAttribute("aria-current")).toBe("page");
    fireEvent.click(screen.getByRole("tab", { name: "Transform" }));
    expect(onSelect).toHaveBeenCalledWith("transform");
  });

  it("renders a per-item badge when the badge fn returns a count", () => {
    render(
      <NavMenu
        items={ITEMS}
        active={null}
        onSelect={() => {}}
        badge={(id) => (id === "overrides" ? 3 : undefined)}
      />,
    );
    expect(screen.getByText("3")).toBeTruthy();
  });

  it("carries a host className onto the .nav-rail root", () => {
    const { container } = render(
      <NavMenu items={ITEMS} active={null} onSelect={() => {}} className="theme-light host-hook" />,
    );
    const root = container.querySelector(".nav-rail");
    expect(root?.className).toContain("theme-light");
    expect(root?.className).toContain("host-hook");
  });
});
