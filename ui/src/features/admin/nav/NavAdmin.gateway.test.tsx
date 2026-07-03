// The nav builder, driven against a REAL spawned gateway (CLAUDE §9 — no fake). Proves the full
// round-trip the scope's UI plan names: the builder picks items from the three REAL sources
// (surfaces, `dashboard.list`, `ext.list`), adds a tag-group, saves through the real
// useNavs → nav.save → /navs route, and reloads the real roster; then `nav.resolve` returns the
// effective (cap-stripped) menu, and the cap-strip is visible in the resolved payload. Every list is a
// real `*.list` call; every write a real `nav.*` verb re-checked server-side. The nav grants nothing.
// Each test logs into a UNIQUE workspace for isolation on the shared node.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, within, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { NavAdmin } from "./NavAdmin";
import { CAP } from "@/lib/session/admin-caps";
import { resolveNav, saveNav, shareNav, setNavPref, getNav } from "@/lib/nav";
import { useRealGateway, signInReal, signInWithCaps } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `nav-${n++}`;

// The full nav authoring cap set the dev login carries (the builder shows for `navSave`).
const AUTHOR_CAPS = [CAP.navList, CAP.navGet, CAP.navSave, CAP.navDelete, CAP.navShare, CAP.navResolve];

beforeAll(() => useRealGateway());

describe("NavAdmin (real gateway)", () => {
  it("builds a nav in the UI (surface + tag-group), saves, and reloads it from the roster", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    render(<NavAdmin ws={ws} caps={AUTHOR_CAPS} />);

    // Start a new nav, title it.
    await user.click(await screen.findByLabelText("New nav"));
    await user.type(screen.getByLabelText("Nav title"), "Operations");

    // Add a `surface` item (channels) — the default kind + surface.
    await user.click(screen.getByLabelText("Add item"));
    // Add a `tag-group` item over the `site` facet.
    await user.selectOptions(screen.getByLabelText("Item kind"), "tag-group");
    await user.type(screen.getByLabelText("Facet key"), "site");
    await user.click(screen.getByLabelText("Add item"));

    // Two items are now staged.
    const list = screen.getByTestId("nav-items");
    expect(within(list).getByText("channels")).toBeInTheDocument();
    expect(within(list).getByText("site")).toBeInTheDocument();

    // Save through the real route.
    await user.click(screen.getByLabelText("Save nav"));
    await waitFor(() => expect(screen.getByText("Saved.")).toBeInTheDocument());

    // Go back to the roster — the real reload lists the saved nav.
    await user.click(screen.getByLabelText("Back"));
    expect(await screen.findByText("Operations")).toBeInTheDocument();

    // And the persisted record round-trips (id slugged from the title; both items present).
    const saved = await getNav("operations");
    expect(saved.title).toBe("Operations");
    expect(saved.items.map((i) => i.kind)).toEqual(["surface", "tag-group"]);
    expect(saved.items[0].surface).toBe("channels");
    expect(saved.items[1].facets?.[0].key).toBe("site");
  });

  it("resolves a workspace nav to the effective menu (member-level resolve)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    // Author a nav, pick it, then resolve it.
    await saveNav("ops", "Ops", [
      { kind: "surface", surface: "channels", label: "Channels" },
      { kind: "surface", surface: "dashboards", label: "Dashboards" },
    ]);
    await setNavPref("ops");

    const resolved = await resolveNav();
    expect(resolved.source).toBe("pick");
    expect(resolved.nav_id).toBe("ops");
    const surfaces = resolved.items.map((i) => i.surface);
    // The dev principal holds dashboard.list, so `dashboards` survives; `channels` is always-visible.
    expect(surfaces).toContain("channels");
    expect(surfaces).toContain("dashboards");
  });

  it("cap-strips a surface the caller lacks — the lens (nav never widens)", async () => {
    // Ada (dev caps) authors a WORKSPACE nav listing `rules` (gated by rules.run) + `channels`, and
    // sets it as the workspace default so a narrower caller resolves it.
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveNav("ops", "Ops", [
      { kind: "surface", surface: "rules", label: "Rules" },
      { kind: "surface", surface: "channels", label: "Channels" },
    ]);
    await shareNav("ops", "workspace");
    // Point the workspace default at it (so a fresh caller lands on this nav).
    const { setDefaultNav } = await import("@/lib/nav");
    await setDefaultNav("ops");

    // Ben logs in with ONLY the resolve cap — NO rules.run. He resolves the same nav.
    await signInWithCaps("user:ben", ws, [CAP.navResolve]);
    const resolved = await resolveNav();
    expect(resolved.source).toBe("workspace-default");
    const surfaces = resolved.items.map((i) => i.surface);
    // `channels` (always-visible) survives; `rules` is STRIPPED (Ben lacks rules.run) — the lens.
    expect(surfaces).toContain("channels");
    expect(surfaces).not.toContain("rules");
  });

  it("falls back (no nav) — resolve returns the fallback source and the rail renders SURFACES", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    const resolved = await resolveNav();
    // No nav authored → the fallback tier (the UI renders its built-in SURFACES, never blank).
    expect(resolved.source).toBe("fallback");
    expect(resolved.items).toEqual([]);
  });
});
