// Membership (global-identity scope), driven against a REAL spawned gateway (CLAUDE §9 — no fake).
// Proves the People-tab roster reads `membership.list` (decision #9): a member added through
// `membership.add` appears in the roster; identity.create + membership.add are reachable for an admin;
// the switcher source `identity.workspaces` resolves the joined set. Each test logs into a UNIQUE
// workspace (the first login bootstraps the requester as workspace-admin) and seeds through the real
// routes.

import { describe, expect, it, beforeAll } from "vitest";

import { addMember, listMembers } from "@/lib/membership/membership.api";
import { createIdentity, identityWorkspaces } from "@/lib/identity/identity.api";
import { useRealGateway, signInReal, signInWithCaps } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `gid-${n++}`;

beforeAll(() => useRealGateway());

describe("membership (real gateway)", () => {
  it("an added member appears in the roster (membership.list)", async () => {
    const ws = nextWs();
    await signInReal("user:alice", ws);
    await createIdentity("user:bob", "Bob");
    await addMember("user:bob");

    const members = await listMembers();
    expect(members.some((m) => m.sub === "user:bob")).toBe(true);
  });

  it("identity.workspaces resolves the workspaces an identity joined", async () => {
    const ws = nextWs();
    await signInReal("user:alice", ws);
    await createIdentity("user:carol");
    await addMember("user:carol");

    const wss = await identityWorkspaces("user:carol");
    expect(wss.some((w) => w.ws === ws)).toBe(true);
  });

  it("a non-admin is denied the membership verbs (server-side)", async () => {
    const ws = nextWs();
    // Bootstrap the workspace via a real login first.
    await signInReal("user:alice", ws);
    // A member with NO manage caps.
    await signInWithCaps("user:eve", ws, ["bus:chan/*:pub"]);
    await expect(listMembers()).rejects.toThrow();
    await expect(addMember("user:dan")).rejects.toThrow();
  });

  it("removing a member drops them from the roster", async () => {
    const ws = nextWs();
    await signInReal("user:alice", ws);
    await createIdentity("user:dave");
    await addMember("user:dave");
    expect((await listMembers()).some((m) => m.sub === "user:dave")).toBe(true);

    const { removeMember } = await import("@/lib/membership/membership.api");
    await removeMember("user:dave");
    expect((await listMembers()).some((m) => m.sub === "user:dave")).toBe(false);
  });
});
