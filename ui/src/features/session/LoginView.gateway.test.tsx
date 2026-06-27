// The session slice in the UI (collaboration scope, slice 1), driven against a REAL spawned gateway
// (no fake — CLAUDE §9). The login FORM obtains a session via the real `login` verb (`POST /login`)
// and `useSession` stores it. `useRealGateway()` points `invoke` at the spawned node, so submitting
// the form fires a real signed token back into the store.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { LoginView } from "./LoginView";
import { useSession } from "@/lib/session";
import { getSession } from "@/lib/session/session.store";
import { useRealGateway } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `login-${n++}`;

beforeAll(() => useRealGateway());

// A tiny harness: render LoginView wired to the real useSession, then assert the store after login.
function Harness() {
  const { signIn } = useSession();
  return <LoginView onSignIn={signIn} />;
}

describe("LoginView (real gateway)", () => {
  it("logs in and stores a real session (token + principal + workspace)", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    render(<Harness />);

    await user.clear(screen.getByLabelText("identity"));
    await user.type(screen.getByLabelText("identity"), "user:ada");
    await user.clear(screen.getByLabelText("workspace"));
    await user.type(screen.getByLabelText("workspace"), ws);
    await user.click(screen.getByLabelText("sign in"));

    // The session store now holds the issued session — every surface scopes to it. The real `POST
    // /login` is async, so wait for the store to settle.
    await waitFor(() => expect(getSession()?.token).toBeTruthy());
    const session = getSession();
    expect(session?.principal).toBe("user:ada");
    expect(session?.workspace).toBe(ws);
  });
});
