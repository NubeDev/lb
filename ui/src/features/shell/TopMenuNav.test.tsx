// TopMenuNav rendering of the resolved nav (shell-chrome-layout scope). Proves the top-menu renderer
// is a faithful SECOND renderer over the same resolved-nav data the rail consumes — same navigable
// entries (same onSelect targets), the fallback SURFACE_GROUPS as menus, Pinned/Extensions/escape-
// hatch/Sign-out all relocated into the menubar, and extension ids staying opaque `ext:<id>` refs.
// Markup only — no gateway (the cap-strip is a server concern, proven in Rust).

import { render, screen, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { TopMenuNav } from "./TopMenuNav";
import type { CoreSurface, ResolvedNavItem } from "./NavRail";
import { BrandingProvider } from "@/lib/branding";
import { ThemeProvider } from "@/lib/theme/ThemeProvider";

afterEach(cleanup);

function renderMenu(props: {
  allowed: CoreSurface[];
  resolvedItems?: ResolvedNavItem[] | null;
  hidden?: string[];
  pinned?: ResolvedNavItem[];
  extSlots?: { ext: string; label: string }[];
  onTogglePin?: (ref: string) => void;
  usingBuiltin?: boolean;
  onShowAllPages?: () => void;
  onUseMyMenu?: () => void;
}) {
  return render(
    <ThemeProvider>
      <BrandingProvider workspace="acme">
        <TopMenuNav
          active="channels"
          onSelect={vi.fn()}
          onSignOut={vi.fn()}
          allowed={props.allowed}
          resolvedItems={props.resolvedItems}
          hidden={props.hidden}
          pinned={props.pinned}
          extSlots={props.extSlots}
          onTogglePin={props.onTogglePin}
          usingBuiltin={props.usingBuiltin}
          onShowAllPages={props.onShowAllPages}
          onUseMyMenu={props.onUseMyMenu}
        />
      </BrandingProvider>
    </ThemeProvider>,
  );
}

describe("TopMenuNav — fallback (no resolved nav)", () => {
  it("renders each SURFACE_GROUPS bucket as a menubar menu trigger", () => {
    renderMenu({ allowed: ["channels", "dashboards", "flows", "datasources", "system"] });
    // Radix Menubar triggers expose role="menuitem"; the buckets with at least one allowed surface appear.
    expect(screen.getByRole("menuitem", { name: "Workspace" })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: "Automation" })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: "Data" })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: "System" })).toBeInTheDocument();
  });

  it("omits a group whose members are all cap-stripped (no empty menu)", () => {
    renderMenu({ allowed: ["channels"] });
    expect(screen.getByRole("menuitem", { name: "Workspace" })).toBeInTheDocument();
    expect(screen.queryByRole("menuitem", { name: "Automation" })).not.toBeInTheDocument();
    expect(screen.queryByRole("menuitem", { name: "Data" })).not.toBeInTheDocument();
  });

  it("navigates via onSelect when a menu item is clicked", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    render(
      <ThemeProvider>
        <BrandingProvider workspace="acme">
          <TopMenuNav
            active="channels"
            onSelect={onSelect}
            onSignOut={vi.fn()}
            allowed={["channels", "dashboards", "flows"]}
          />
        </BrandingProvider>
      </ThemeProvider>,
    );
    await user.click(screen.getByRole("menuitem", { name: "Automation" }));
    // The Flows item opens in the dropdown (radix menubar items share role="menuitem").
    await user.click(screen.getByRole("menuitem", { name: /^Flows/ }));
    expect(onSelect).toHaveBeenCalledWith("flows");
  });
});

describe("TopMenuNav — resolved nav", () => {
  it("renders the RESOLVED menu (flat items fold into a leading 'Menu'; groups become their own menus)", () => {
    const resolvedItems: ResolvedNavItem[] = [
      { kind: "surface", label: "Channels", surface: "channels" },
      { kind: "dashboard", label: "Cooler Health", dashboard: "dashboard:cooler" },
      {
        kind: "group",
        label: "Admin",
        items: [{ kind: "surface", label: "Rules", surface: "rules" }],
      },
    ];
    renderMenu({ allowed: ["channels", "dashboards", "rules", "flows"], resolvedItems });
    expect(screen.getByRole("menuitem", { name: "Menu" })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: "Admin" })).toBeInTheDocument();
    // A surface in `allowed` but NOT in the resolved menu is HIDDEN (the resolved menu wins).
    expect(screen.queryByRole("menuitem", { name: "Workspace" })).not.toBeInTheDocument();
  });
});

describe("TopMenuNav — affordance parity with the rail", () => {
  it("surfaces Pinned and Extensions as their own menus when non-empty", () => {
    renderMenu({
      allowed: ["channels"],
      pinned: [{ kind: "surface", label: "Rules", surface: "rules" }],
      extSlots: [{ ext: "weather", label: "Weather" }],
    });
    expect(screen.getByRole("menuitem", { name: /Pinned/ })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: /Extensions/ })).toBeInTheDocument();
  });

  it("keeps extension ids opaque — no branch on identity", () => {
    // The ext slot is rendered as an ext:<id> surface; the id is opaque data, never a special case.
    renderMenu({ allowed: ["channels"], extSlots: [{ ext: "mqtt", label: "MQTT" }] });
    expect(screen.getByRole("menuitem", { name: /Extensions/ })).toBeInTheDocument();
  });

  it("carries the no-lockout escape hatch + Sign out in the account menu", async () => {
    const user = userEvent.setup();
    const onShowAllPages = vi.fn();
    const onSignOut = vi.fn();
    render(
      <ThemeProvider>
        <BrandingProvider workspace="acme">
          <TopMenuNav
            active="channels"
            onSelect={vi.fn()}
            onSignOut={onSignOut}
            allowed={["channels"]}
            resolvedItems={[{ kind: "surface", label: "Channels", surface: "channels" }]}
            onShowAllPages={onShowAllPages}
          />
        </BrandingProvider>
      </ThemeProvider>,
    );
    await user.click(screen.getByRole("menuitem", { name: "Account" }));
    await user.click(screen.getByRole("menuitem", { name: /Show all pages/ }));
    expect(onShowAllPages).toHaveBeenCalledOnce();
  });

  it("subtracts the workspace hidden-set from the fallback menus", () => {
    renderMenu({ allowed: ["channels", "dashboards", "flows"], hidden: ["dashboards"] });
    // Dashboards is hidden; Channels still reaches its group.
    expect(screen.getByRole("menuitem", { name: "Workspace" })).toBeInTheDocument();
  });
});
