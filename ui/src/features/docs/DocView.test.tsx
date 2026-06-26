// The S4 sharing exit gate, at the UI level: a doc shared to a team is readable by a team
// member, and a NON-member sees the node's "denied" — the same gate the Rust `assets_doc_test`
// proves on the backend, surfaced through the real api client + the faithful in-memory fake.
//
// We drive the actual `assets.api` → `invoke` → fake path (no mock of the api): the fake mirrors
// the node's membership gate, so this exercises the allow/deny branches the user actually hits.

import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { DocView } from "./DocView";
import { putDoc, shareDoc } from "@/lib/assets/assets.api";
import { __seedMembership, __resetAssetsFake } from "@/lib/ipc/assets.fake";

const WS = "acme";

beforeEach(async () => {
  __resetAssetsFake();
  // Ada owns a doc and shares it to team:engineering; Ben is a member, Cleo is not.
  await putDoc(WS, "scope-x", "Scope X", "the draft body", 1, "user:ada");
  await shareDoc(WS, "scope-x", "team:engineering");
  __seedMembership(WS, { members: { "team:engineering": ["user:ben"] } });
});

afterEach(() => __resetAssetsFake());

describe("DocView sharing gate", () => {
  it("a team member sees the shared doc content", async () => {
    render(<DocView ws={WS} id="scope-x" author="user:ben" />);
    await waitFor(() => expect(screen.getByText("the draft body")).toBeInTheDocument());
  });

  it("a non-member is denied (the gate-3 membership deny, surfaced to the user)", async () => {
    render(<DocView ws={WS} id="scope-x" author="user:cleo" />);
    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent("don't have access"),
    );
    expect(screen.queryByText("the draft body")).not.toBeInTheDocument();
  });

  it("the owner always sees their own doc", async () => {
    render(<DocView ws={WS} id="scope-x" author="user:ada" />);
    await waitFor(() => expect(screen.getByText("the draft body")).toBeInTheDocument());
  });
});
