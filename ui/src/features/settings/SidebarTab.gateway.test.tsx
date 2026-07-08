// The Settings → Sidebar tab + pins, driven against a REAL spawned gateway (CLAUDE §9 — no fake;
// hide-and-pins scope). Proves the UI round-trip: an admin toggles a surface hidden and saves
// through the real `/nav/hidden` route; `nav.resolve` echoes the set on EVERY tier (including the
// fallback the client subtracts from); a member without `nav.save` cannot write it; pins ride the
// member-owned `nav_pref` (a partial write that never clobbers the active pick); and hide beats pin
// — un-hiding restores the pin with no `nav_pref` rewrite. Unique workspace per test for isolation.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { SidebarTab } from "./SidebarTab";
import { CAP } from "@/lib/session/admin-caps";
import {
  getNavHidden,
  getNavPref,
  resolveNav,
  setNavHidden,
  setNavPins,
  setNavPref,
  saveNav,
} from "@/lib/nav";
import { useRealGateway, signInReal, signInWithCaps } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `hidepin-${n++}`;

const ADMIN_CAPS = [CAP.navSave, CAP.navResolve, CAP.navList, CAP.navGet];

beforeAll(() => useRealGateway());

describe("Sidebar hide + pins (real gateway)", () => {
  it("admin hides a surface through the tab; the hidden-set persists and echoes on resolve", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    render(<SidebarTab ws={ws} caps={ADMIN_CAPS} />);

    // Toggle Dashboards hidden and save through the real POST /nav/hidden.
    const toggle = await screen.findByLabelText("Hide Dashboards");
    await waitFor(() => expect(toggle).toBeEnabled());
    await user.click(toggle);
    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(screen.getByText("Saved.")).toBeInTheDocument());

    // Persisted: the record holds the ref; the resolver echoes it even on the FALLBACK tier
    // (no nav authored) — that echo is what the rail subtracts from its client-side menu.
    expect((await getNavHidden()).hidden).toEqual(["dashboards"]);
    const resolved = await resolveNav();
    expect(resolved.source).toBe("fallback");
    expect(resolved.hidden).toEqual(["dashboards"]);
  });

  it("a member without nav.save cannot write the hidden-set (deny per verb; read still works)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await setNavHidden(["rules"]);

    await signInWithCaps("user:ben", ws, [CAP.navResolve]);
    // The member READS the set (it shapes his rail)…
    expect((await getNavHidden()).hidden).toEqual(["rules"]);
    // …but his write is refused server-side, and nothing changed.
    await expect(setNavHidden([])).rejects.toThrow();
    expect((await getNavHidden()).hidden).toEqual(["rules"]);
  });

  it("hidden refs strip from a resolved menu server-side (every tier, not just fallback)", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveNav("ops", "Ops", [
      { kind: "surface", surface: "channels", label: "Channels" },
      { kind: "surface", surface: "inbox", label: "Inbox" },
    ]);
    await setNavPref("ops");
    await setNavHidden(["inbox"]);

    const resolved = await resolveNav();
    expect(resolved.source).toBe("pick");
    const surfaces = resolved.items.map((i) => i.surface);
    expect(surfaces).toContain("channels");
    expect(surfaces).not.toContain("inbox"); // hidden-stripped server-side
  });

  it("pins ride nav_pref (partial write — the active pick survives), resolve returns them in order", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveNav("ops", "Ops", [{ kind: "surface", surface: "channels", label: "Channels" }]);
    await setNavPref("ops");

    // A pin-only write must NOT clobber the active pick.
    await setNavPins(["inbox", "channels"]);
    const pref = await getNavPref();
    expect(pref.active).toBe("ops");
    expect(pref.pinned).toEqual(["inbox", "channels"]);

    const resolved = await resolveNav();
    expect((resolved.pinned ?? []).map((i) => i.surface)).toEqual(["inbox", "channels"]);
  });

  it("hide beats pin, and un-hiding restores the pin with no nav_pref rewrite", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await setNavPins(["channels"]);
    expect(((await resolveNav()).pinned ?? []).length).toBe(1);

    await setNavHidden(["channels"]);
    expect((await resolveNav()).pinned ?? []).toEqual([]);
    // The stored record still holds the pin — the strip never mutates it.
    expect((await getNavPref()).pinned).toEqual(["channels"]);

    await setNavHidden([]);
    expect(((await resolveNav()).pinned ?? []).map((i) => i.surface)).toEqual(["channels"]);
  });
});
