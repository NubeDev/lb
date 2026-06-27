// App-shell cap-gated nav (admin-console scope): the Admin + Extensions surfaces appear in the nav
// only for a session whose token carries the admin caps; a plain member never sees them. The gateway
// re-checks every verb regardless — this asserts only the convenience display gate (the server deny
// on a forged call is proven in Rust, role/gateway/tests/admin_routes_test.rs).

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import { App } from "./App";
import { CAP } from "@/lib/session/admin-caps";
import { setSession } from "@/lib/session/session.store";
import { allowedSurfaces } from "@/features/routing/allowed";

const MEMBER_CAPS = ["bus:chan/*:pub", "bus:chan/*:sub", "mcp:members.list:call"];

beforeEach(() => {
  window.history.replaceState(null, "", "/#/channels?c=general");
  setSession(null);
});
afterEach(() => setSession(null));

describe("App nav cap-gating", () => {
  it("keeps the logged-out short-circuit before routes render", async () => {
    render(<App />);
    expect(await screen.findByRole("heading", { name: "Sign in" })).toBeInTheDocument();
  });

  it("allows Admin for an admin cap and hides it for a plain member", () => {
    expect(allowedSurfaces([CAP.userManage])).toContain("admin");
    expect(allowedSurfaces(MEMBER_CAPS)).not.toContain("admin");
  });

  it("includes Extensions when the session holds ext.list", () => {
    expect(allowedSurfaces([CAP.extList])).toContain("extensions");
  });
});
