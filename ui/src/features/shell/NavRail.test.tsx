// NavRail rendering of a resolved nav (nav scope). Proves the shell's half of the lens: when
// `nav.resolve` returns a menu, NavRail renders THOSE items (already cap-stripped server-side) instead
// of the built-in `SURFACES`; when it returns nothing (fallback), NavRail renders the built-in
// `SURFACES.filter(allowed)` — never a blank rail. The cap-strip itself is a SERVER concern (proven in
// the Rust `nav_test.rs` "nav never widens" test); here we prove the rail faithfully renders whatever
// the (already-stripped) resolve payload contains, and falls back correctly. Markup only — no gateway.

import { render, screen, cleanup } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { NavRail, type CoreSurface, type ResolvedNavItem } from "./NavRail";
import { SidebarProvider } from "@/components/ui/sidebar";

afterEach(cleanup);

function renderRail(props: {
  allowed: CoreSurface[];
  resolvedItems?: ResolvedNavItem[] | null;
}) {
  return render(
    <SidebarProvider>
      <NavRail
        active="channels"
        onSelect={vi.fn()}
        onSignOut={vi.fn()}
        allowed={props.allowed}
        resolvedItems={props.resolvedItems}
      />
    </SidebarProvider>,
  );
}

describe("NavRail resolved-nav rendering", () => {
  it("renders the built-in SURFACES fallback when no nav resolves (resolvedItems null)", () => {
    renderRail({ allowed: ["channels", "dashboards", "settings"], resolvedItems: null });
    // The allowed built-ins render…
    expect(screen.getByRole("button", { name: "Channels" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Dashboards" })).toBeInTheDocument();
    // …and a NON-allowed built-in does not (the display cap-gate).
    expect(screen.queryByRole("button", { name: "Rules" })).not.toBeInTheDocument();
  });

  it("renders the RESOLVED menu (already cap-stripped) instead of SURFACES when a nav applies", () => {
    // The resolve payload lists only channels + a dashboard + a group with rules — NOT the full
    // SURFACES set. NavRail must render exactly these, proving it renders the lens, not the built-ins.
    const resolvedItems: ResolvedNavItem[] = [
      { kind: "surface", label: "Channels", surface: "channels" },
      { kind: "dashboard", label: "Cooler Health", dashboard: "dashboard:cooler" },
      {
        kind: "group",
        label: "Admin",
        items: [{ kind: "surface", label: "Rules", surface: "rules" }],
      },
    ];
    // `allowed` includes many surfaces, but the resolved menu overrides it — those extras must NOT show.
    renderRail({
      allowed: ["channels", "dashboards", "rules", "flows", "system", "settings"],
      resolvedItems,
    });

    expect(screen.getByRole("button", { name: "Channels" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cooler Health" })).toBeInTheDocument();
    // The group header + its child render.
    expect(screen.getByText("Admin")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Rules" })).toBeInTheDocument();
    // A surface in `allowed` but NOT in the resolved menu is HIDDEN (the resolved menu wins).
    expect(screen.queryByRole("button", { name: "Flows" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "System" })).not.toBeInTheDocument();
  });

  it("renders the visible-but-stripped shape: a cap-stripped item simply is not in the payload", () => {
    // Server-side, `nav.resolve` already removed the entries the caller can't reach. The rail renders
    // whatever survived — so a menu that authored [channels, rules] but where the caller lacked rules
    // arrives as just [channels]. NavRail shows channels and NOT rules — the lens, faithfully rendered.
    const stripped: ResolvedNavItem[] = [{ kind: "surface", label: "Channels", surface: "channels" }];
    renderRail({ allowed: ["channels", "rules"], resolvedItems: stripped });
    expect(screen.getByRole("button", { name: "Channels" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Rules" })).not.toBeInTheDocument();
  });

  it("falls back to SURFACES (never blank) when the resolved menu is empty", () => {
    renderRail({ allowed: ["channels", "settings"], resolvedItems: [] });
    expect(screen.getByRole("button", { name: "Channels" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Settings" })).toBeInTheDocument();
  });
});
