// The S4 sharing exit gate at the UI level, driven against a REAL spawned gateway node (no fake —
// CLAUDE §9 / testing §0). A doc is created + shared + team membership populated through the REAL
// `assets.*` / members api clients (the same `/docs*` + `/teams/{team}/members` routes production
// uses), then `DocView` is rendered from each reader's REAL session. This exercises the host's three
// gates server-side: a team member sees the content; a NON-member is denied (the gate-3 membership
// deny, surfaced as the node's opaque "denied" → the view's "don't have access"); the owner always
// reads their own doc.
//
// Each principal renders the view from its OWN signed session (the gateway derives the reader from the
// token, never the `author` prop — the hard wall, §7), all within ONE unique workspace per test so the
// share→member edges resolve. The corresponding backend isolation is proven in the Rust
// `role/gateway/tests/assets_workflow_routes_test.rs` (`ws_b_session_cannot_read_ws_a_doc`).

import { beforeAll, describe, expect, it } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

import { DocView } from "./DocView";
import { putDoc, shareDoc } from "@/lib/assets/assets.api";
import { addMember } from "@/lib/members/members.api";
import { addMember as addWorkspaceMember } from "@/lib/membership/membership.api";
import { useRealGateway, signInReal } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `docs-${n++}`;

beforeAll(() => useRealGateway());

/** Seed (as the owner) a doc shared to `team:engineering` with `user:ben` a real member of it.
 *  Returns the workspace. Every write is a real route call behind the workspace wall. */
async function seedSharedDoc(ws: string): Promise<void> {
  await signInReal("user:ada", ws);
  // global-identity: ben + cleo must be WORKSPACE members to log in (decision #4). Ada (the
  // bootstrapped workspace-admin) adds them before they sign in below.
  await addWorkspaceMember("user:ben");
  await addWorkspaceMember("user:cleo");
  await putDoc(ws, "scope-x", "Scope X", "the draft body", 1, "user:ada");
  await shareDoc(ws, "scope-x", "team:engineering");
  await addMember("team:engineering", "user:ben"); // a REAL `member` edge (gated by store:doc/*:write)
}

describe("DocView sharing gate (real gateway)", () => {
  it("a team member sees the shared doc content", async () => {
    const ws = nextWs();
    await seedSharedDoc(ws);

    // Ben reads from his OWN session in the same workspace — gate 3 passes via team membership.
    await signInReal("user:ben", ws);
    render(<DocView ws={ws} id="scope-x" author="user:ben" />);
    await waitFor(() => expect(screen.getByText("the draft body")).toBeInTheDocument());
  });

  it("a non-member is denied (the gate-3 membership deny, surfaced to the user)", async () => {
    const ws = nextWs();
    await seedSharedDoc(ws);

    // Cleo is in the workspace (her own session) but not in the team → the host returns `denied`.
    await signInReal("user:cleo", ws);
    render(<DocView ws={ws} id="scope-x" author="user:cleo" />);
    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent("don't have access"),
    );
    expect(screen.queryByText("the draft body")).not.toBeInTheDocument();
  });

  it("the owner always sees their own doc", async () => {
    const ws = nextWs();
    await seedSharedDoc(ws); // leaves us signed in as the owner (user:ada)
    render(<DocView ws={ws} id="scope-x" author="user:ada" />);
    await waitFor(() => expect(screen.getByText("the draft body")).toBeInTheDocument());
  });
});
