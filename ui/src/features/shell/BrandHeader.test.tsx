// BrandHeader — proves the brand identity row wiring survives the motion extraction: the mark +
// name/tagline render, the row is the collapse toggle when collapsible, and a plain static brand in
// `none` mode. Motion is gated by the seam (useMotionPref); with the default theme (motion on) the
// row still renders identical content — the animation is transform/opacity only, so the DOM the tests
// assert on is unchanged. Markup only — no gateway.

import { render, screen, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { BrandHeader } from "./BrandHeader";
import { SidebarProvider } from "@/components/ui/sidebar";
import { ThemeProvider } from "@/lib/theme/ThemeProvider";

afterEach(cleanup);

const BRAND = { siteName: "Nube IO", siteAbbr: "IO", tagline: "is cool" };

function renderHeader(props: { canToggle: boolean; onToggle?: () => void }) {
  return render(
    <ThemeProvider>
      <SidebarProvider>
        <BrandHeader
          brand={BRAND}
          canToggle={props.canToggle}
          onToggle={props.onToggle ?? vi.fn()}
          toggleLabel="Collapse sidebar"
        />
      </SidebarProvider>
    </ThemeProvider>,
  );
}

describe("BrandHeader", () => {
  it("renders the brand name, tagline, and fallback abbr tile", () => {
    renderHeader({ canToggle: true });
    expect(screen.getByText("Nube IO")).toBeInTheDocument();
    expect(screen.getByText("is cool")).toBeInTheDocument();
    // No logo/icon image ⇒ the text abbr tile is the mark.
    expect(screen.getByText("IO")).toBeInTheDocument();
  });

  it("is the collapse toggle when collapsible", async () => {
    const onToggle = vi.fn();
    renderHeader({ canToggle: true, onToggle });
    const btn = screen.getByRole("button", { name: "Collapse sidebar" });
    await userEvent.click(btn);
    expect(onToggle).toHaveBeenCalledOnce();
  });

  it("renders a plain static brand (no button) in none mode", () => {
    renderHeader({ canToggle: false });
    expect(screen.queryByRole("button", { name: "Collapse sidebar" })).toBeNull();
    // Content is still present — just not interactive.
    expect(screen.getByText("Nube IO")).toBeInTheDocument();
  });
});
