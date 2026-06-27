// The session slice in the UI (collaboration scope, slice 1): the login form obtains a session via
// the (fake) `login` verb, and `useSession` stores it. Mirrors ChannelView.test.tsx in shape.

import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { LoginView } from "./LoginView";
import { useSession } from "@/lib/session";
import { getSession } from "@/lib/session/session.store";

// A tiny harness: render LoginView wired to the real useSession, then assert the store after login.
function Harness() {
  const { signIn } = useSession();
  return <LoginView onSignIn={signIn} />;
}

describe("LoginView", () => {
  it("logs in and stores a real session (token + principal + workspace)", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    await user.clear(screen.getByLabelText("identity"));
    await user.type(screen.getByLabelText("identity"), "user:ada");
    await user.clear(screen.getByLabelText("workspace"));
    await user.type(screen.getByLabelText("workspace"), "acme");
    await user.click(screen.getByLabelText("sign in"));

    // The session store now holds the issued session — every surface scopes to it.
    await screen.findByLabelText("sign in"); // settle
    const session = getSession();
    expect(session?.principal).toBe("user:ada");
    expect(session?.workspace).toBe("acme");
    expect(session?.token).toBeTruthy();
  });
});
