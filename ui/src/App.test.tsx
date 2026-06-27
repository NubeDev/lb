// App-shell cap-gated nav (admin-console scope): the Admin + Extensions surfaces appear in the nav
// only for a session whose token carries the admin caps; a plain member never sees them. The gateway
// re-checks every verb regardless — this asserts only the convenience display gate (the server deny
// on a forged call is proven in Rust, role/gateway/tests/admin_routes_test.rs).

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import { App } from "./App";
import { ADMIN_CAPS } from "@/lib/session/admin-caps";
import { setSession } from "@/lib/session/session.store";

const MEMBER_CAPS = ["bus:chan/*:pub", "bus:chan/*:sub", "mcp:members.list:call"];

beforeEach(() => setSession(null));
afterEach(() => setSession(null));

describe("App nav cap-gating", () => {
  it("an admin session sees the Admin and Extensions nav entries", async () => {
    setSession({ token: "t", principal: "user:ada", workspace: "acme", caps: ADMIN_CAPS });
    render(<App />);
    expect(await screen.findByLabelText("Admin")).toBeInTheDocument();
    expect(screen.getByLabelText("Extensions")).toBeInTheDocument();
  });

  it("a plain member never sees the Admin or Extensions nav entries", async () => {
    setSession({ token: "t", principal: "user:bob", workspace: "acme", caps: MEMBER_CAPS });
    render(<App />);
    // Channels is always present (sanity the shell rendered).
    expect(await screen.findByLabelText("Channels")).toBeInTheDocument();
    expect(screen.queryByLabelText("Admin")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Extensions")).not.toBeInTheDocument();
  });
});
