// Regression (agent-catalog / session): a `401` from the gateway on an AUTHENTICATED request means
// the stored session token no longer verifies — classically because the node re-keyed on restart
// (its signing key was ephemeral before `signing-seed` persistence). The IPC layer must clear the
// session so the app falls back to login, NOT leave a logged-in shell whose every read silently
// rejected to an empty state (the "No agent definitions available" symptom over a store that still
// held the 6 seeded defs). A `403` (capability Denied) must be left alone — the caller is still a
// valid session, just lacks the cap.
//
// The `Response` here is the real browser primitive; only `fetch`'s endpoint is stubbed (we are
// testing client-side error MAPPING, not node behaviour — no re-implemented backend, rule 9).

import { afterEach, describe, expect, it, vi } from "vitest";

import { httpInvoke } from "./http";
import { getSession, setSession } from "@/lib/session/session.store";
import type { Session } from "@/lib/session/session.types";

const SESSION: Session = { token: "stale.jwt.token", principal: "user:ada", workspace: "acme" };

afterEach(() => {
  setSession(null);
  vi.unstubAllGlobals();
});

describe("gateway 401 handling", () => {
  it("clears the session when an authenticated request 401s (stale token → fall back to login)", async () => {
    setSession(SESSION);
    vi.stubGlobal("fetch", async () => new Response("token expired", { status: 401 }));

    await expect(httpInvoke("dashboard_list")).rejects.toThrow();
    // The stale session is dropped — the shell will render the login screen, not silent-empty panels.
    expect(getSession()).toBeNull();
  });

  it("keeps the session on a 403 (capability Denied — the caller is still authenticated)", async () => {
    setSession(SESSION);
    vi.stubGlobal("fetch", async () => new Response("not permitted", { status: 403 }));

    await expect(httpInvoke("dashboard_list")).rejects.toThrow();
    // A 403 is an authorization failure, not an authentication one — do NOT log the user out.
    expect(getSession()).toEqual(SESSION);
  });
});
