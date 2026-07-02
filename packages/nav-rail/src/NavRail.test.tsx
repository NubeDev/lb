// Real-component tests for NavRail — renders the actual component with real `items` and
// asserts real DOM/interaction (no fakes, per CLAUDE §9). jsdom has no matchMedia, so
// useIsMobile reports desktop and labels render (expanded mode).

import { fireEvent, render, screen } from "@testing-library/react";
import { Boxes, Braces } from "lucide-react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import { NavRail } from "./NavRail";
import type { NavItem } from "./items";

const ITEMS: NavItem[] = [
  { id: "components", label: "Components", icon: Boxes },
  { id: "scripts", label: "Scripts", icon: Braces, group: "Author" },
  { id: "schedule", label: "Schedule", group: "Author" },
];

function btn(label: string) {
  return screen.getByRole("button", { name: label });
}

describe("NavRail", () => {
  it("renders one button per item, in array order, grouped by `group`", () => {
    render(<NavRail items={ITEMS} active={null} onSelect={() => {}} />);
    for (const it of ITEMS) expect(btn(it.label)).toBeTruthy();

    // Group label present for the "Author" bucket.
    expect(screen.getByText("Author")).toBeTruthy();

    // Array order preserved across the whole rail.
    const wanted = new Set(ITEMS.map((i) => i.label));
    const menuButtons = screen
      .getAllByRole("button")
      .map((b) => b.getAttribute("aria-label"))
      .filter((l) => l && wanted.has(l));
    expect(menuButtons).toEqual(["Components", "Scripts", "Schedule"]);
  });

  it("calls onSelect with the item id on click", () => {
    const onSelect = vi.fn();
    render(<NavRail items={ITEMS} active={null} onSelect={onSelect} />);
    fireEvent.click(btn("Scripts"));
    expect(onSelect).toHaveBeenCalledWith("scripts");
  });

  it("marks the active item aria-current=page and nothing else", () => {
    render(<NavRail items={ITEMS} active="components" onSelect={() => {}} />);
    expect(btn("Components").getAttribute("aria-current")).toBe("page");
    expect(btn("Scripts").getAttribute("aria-current")).toBeNull();
  });

  it("drives selection through a controlled host", () => {
    function Host() {
      const [sel, setSel] = useState<string | null>(null);
      return <NavRail items={ITEMS} active={sel} onSelect={setSel} />;
    }
    render(<Host />);
    fireEvent.click(btn("Schedule"));
    expect(btn("Schedule").getAttribute("aria-current")).toBe("page");
  });

  it("renders header/footer slots only when provided", () => {
    const { rerender } = render(<NavRail items={ITEMS} active={null} onSelect={() => {}} />);
    expect(screen.queryByText("BRAND")).toBeNull();
    expect(screen.queryByText("FOOT")).toBeNull();

    rerender(
      <NavRail
        items={ITEMS}
        active={null}
        onSelect={() => {}}
        header={<div>BRAND</div>}
        footer={<div>FOOT</div>}
      />,
    );
    expect(screen.getByText("BRAND")).toBeTruthy();
    expect(screen.getByText("FOOT")).toBeTruthy();
  });

  it("passes a host className / token override onto the .nav-rail root", () => {
    const { container } = render(
      <NavRail
        items={ITEMS}
        active={null}
        onSelect={() => {}}
        className="theme-light host-hook"
      />,
    );
    const root = container.querySelector(".nav-rail");
    expect(root).toBeTruthy();
    expect(root!.className).toContain("theme-light");
    expect(root!.className).toContain("host-hook");
  });

  it("toggles collapsed state on ⌘/Ctrl-B", () => {
    render(<NavRail items={ITEMS} active={null} onSelect={() => {}} />);
    // Expanded by default → the desktop sidebar wrapper reports data-state=expanded.
    const stateEl = () => document.querySelector('[data-slot="sidebar"]');
    expect(stateEl()?.getAttribute("data-state")).toBe("expanded");
    fireEvent.keyDown(window, { key: "b", ctrlKey: true });
    expect(stateEl()?.getAttribute("data-state")).toBe("collapsed");
  });

  it("respects defaultCollapsed", () => {
    render(<NavRail items={ITEMS} active={null} onSelect={() => {}} defaultCollapsed />);
    expect(document.querySelector('[data-slot="sidebar"]')?.getAttribute("data-state")).toBe(
      "collapsed",
    );
  });
});
