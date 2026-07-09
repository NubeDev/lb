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
import { resolveNav, saveNav, shareNav, listNavShares, setNavPref, getNav } from "@/lib/nav";
import { addMember } from "@/lib/members/members.api";
import { createTeam } from "@/lib/admin/teams.api";
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

  it("lists and removes team shares through the builder (the add/remove team surface)", async () => {
    // Ada authors a nav, shares it to TWO teams (each call writes one S4 share edge), then opens
    // the builder and sees both in the share roster. She removes one via the UI; the surviving
    // team's member still resolves the nav, the removed team's member falls through to the
    // fallback. Proves the round-trip the rust `share_roster_*` tests cover, over the real gateway.
    const ws = nextWs();
    await signInReal("user:ada", ws);
    await saveNav("ops", "Ops", [{ kind: "surface", surface: "channels", label: "Channels" }]);

    // Put ben in team:ops, cleo in team:eng (each via the real members_add edge write).
    await addMember("team:ops", "user:ben");
    await addMember("team:eng", "user:cleo");
    // Two share edges accumulate (relate is multi-edge).
    await shareNav("ops", "team", "team:ops");
    await shareNav("ops", "team", "team:eng");

    // The API client sees both.
    let shares = await listNavShares("ops");
    expect(shares.sort()).toEqual(["team:eng", "team:ops"]);

    // Ben resolves the nav via team:ops before the unshare.
    await signInWithCaps("user:ben", ws, [CAP.navResolve]);
    expect((await resolveNav()).nav_id).toBe("ops");

    // Back to Ada: open the builder, the roster renders both, remove team:ops from the UI.
    await signInReal("user:ada", ws);
    const user = userEvent.setup();
    render(<NavAdmin ws={ws} caps={AUTHOR_CAPS} />);
    await user.click(await screen.findByLabelText("Edit Ops"));
    const roster = await screen.findByTestId("nav-shares");
    await waitFor(() => expect(within(roster).getByText("team:ops")).toBeInTheDocument());
    expect(within(roster).getByText("team:eng")).toBeInTheDocument();

    await user.click(within(roster).getByLabelText("Remove share to team:ops"));
    await waitFor(() =>
      expect(within(roster).queryByText("team:ops")).not.toBeInTheDocument(),
    );

    // The API confirms: only team:eng survives.
    shares = await listNavShares("ops");
    expect(shares).toEqual(["team:eng"]);

    // Ben no longer resolves the nav (his team's share was revoked); cleo still does.
    await signInWithCaps("user:ben", ws, [CAP.navResolve]);
    expect((await resolveNav()).source).toBe("fallback");
    await signInWithCaps("user:cleo", ws, [CAP.navResolve]);
    expect((await resolveNav()).nav_id).toBe("ops");
  });

  it("closes the private-by-default bug: authored-but-unshared nav is invisible, one-click team share fixes it", async () => {
    // The live bug this UX rewrite fixes: an admin builds a nav, Saves, and walks away — the nav is
    // `private` (its default visibility), so it's invisible to everyone (nav `main` shipped private
    // with zero team shares → bob saw nothing). This proves (a) the state is real and (b) the new
    // "Who sees this nav" section makes it obvious AND one-click-fixable: picking a team both switches
    // the nav to the Team tier and writes the share edge, after which a member of that team resolves it.
    const ws = nextWs();
    await signInReal("user:ada", ws);
    // Seed a real team RECORD (so the picker has a real option) + a member (so there's someone to
    // resolve it). Seed BEFORE render — the builder loads `teams.list` once on mount.
    await createTeam("team:ops", "Operations");
    await addMember("team:ops", "user:ben");

    const user = userEvent.setup();
    render(<NavAdmin ws={ws} caps={AUTHOR_CAPS} />);

    // Author a nav and Save it — but DON'T assign any audience (the walk-away path).
    await user.click(await screen.findByLabelText("New nav"));
    await user.type(screen.getByLabelText("Nav title"), "Main");
    await user.click(screen.getByLabelText("Add item")); // a default `channels` surface
    await user.click(screen.getByLabelText("Save nav"));
    await waitFor(() => expect(screen.getByText("Saved.")).toBeInTheDocument());

    // The saved nav is `private` — the persisted record proves the invisible-by-default state.
    expect((await getNav("main")).visibility).toBe("private");
    // And the UI says so, in one obvious line (not buried in a dropdown).
    const shares = await screen.findByTestId("nav-shares");
    expect(within(shares).getByText(/Only you \(private\)/)).toBeInTheDocument();

    // Ben (in team:ops) cannot resolve it yet — private = invisible to everyone but the owner.
    await signInWithCaps("user:ben", ws, [CAP.navResolve]);
    expect((await resolveNav()).source).toBe("fallback");

    // Back to Ada: ONE click on the team picker fixes it (switch to Team tier + write the edge).
    await signInReal("user:ada", ws);
    await user.selectOptions(within(shares).getByLabelText("Team to add"), "team:ops");
    await user.click(within(shares).getByLabelText("Add team share"));
    // The roster shows the team NAME (not the raw id) via `teamName`.
    await waitFor(() => expect(within(shares).getByText("Operations")).toBeInTheDocument());
    expect((await getNav("main")).visibility).toBe("team");

    // Now Ben resolves the nav via his team — the bug is closed.
    await signInWithCaps("user:ben", ws, [CAP.navResolve]);
    expect((await resolveNav()).nav_id).toBe("main");
  });
});
