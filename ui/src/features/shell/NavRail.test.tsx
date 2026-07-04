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
import { ThemeProvider } from "@/lib/theme/ThemeProvider";
import { THEME_STORAGE_KEY } from "@/lib/theme";

afterEach(cleanup);

function renderRail(props: {
  allowed: CoreSurface[];
  resolvedItems?: ResolvedNavItem[] | null;
}) {
  return render(
    <ThemeProvider>
      <SidebarProvider>
        <NavRail
          active="channels"
          onSelect={vi.fn()}
          onSignOut={vi.fn()}
          allowed={props.allowed}
          resolvedItems={props.resolvedItems}
        />
      </SidebarProvider>
    </ThemeProvider>,
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

  it("groups the fallback rail into labelled categories, hiding a fully cap-stripped group", () => {
    // channels+dashboards (Workspace) and flows (Automation) are allowed; nothing in Data/Build/System.
    renderRail({ allowed: ["channels", "dashboards", "flows"], resolvedItems: null });
    // The category labels for groups WITH visible members render…
    expect(screen.getByText("Workspace")).toBeInTheDocument();
    expect(screen.getByText("Automation")).toBeInTheDocument();
    // …and a group whose members are all cap-stripped renders no label at all.
    expect(screen.queryByText("Data")).not.toBeInTheDocument();
    expect(screen.queryByText("Build")).not.toBeInTheDocument();
    expect(screen.queryByText("System")).not.toBeInTheDocument();
    // The old flat "Core" label is gone.
    expect(screen.queryByText("Core")).not.toBeInTheDocument();
  });

  it("shows a single merged 'Studio' rail entry when EITHER extensions or studio (build) is allowed", () => {
    // Build-only session (studio cap, no extensions cap): the merged entry still shows — its click
    // lands on the first tab the caps allow. There is no separate 'Extensions' rail entry.
    renderRail({ allowed: ["channels", "studio"], resolvedItems: null });
    expect(screen.getByText("Build")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Studio" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Extensions" })).not.toBeInTheDocument();
  });

  it("hides the 'Build' group when neither extensions, studio, nor data-studio is allowed", () => {
    renderRail({ allowed: ["channels", "dashboards"], resolvedItems: null });
    expect(screen.queryByText("Build")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Studio" })).not.toBeInTheDocument();
  });

  it("spreads the member's theme layout (variant/side) onto the shadcn <Sidebar>", () => {
    // Seed a non-default sidebar layout into the theme cache; NavRail reads it via useTheme and passes
    // it to <Sidebar>, which reflects it as data-variant / data-side on the rail element.
    localStorage.setItem(
      THEME_STORAGE_KEY,
      JSON.stringify({
        mode: "dark",
        preset: "amber",
        radius: "0.5rem",
        layout: { variant: "floating", collapsible: "icon", side: "right" },
      }),
    );
    const { container } = renderRail({ allowed: ["channels"], resolvedItems: null });
    const sidebar = container.querySelector("[data-variant]");
    expect(sidebar).not.toBeNull();
    expect(sidebar).toHaveAttribute("data-variant", "floating");
    expect(sidebar).toHaveAttribute("data-side", "right");
    localStorage.clear();
  });
});
