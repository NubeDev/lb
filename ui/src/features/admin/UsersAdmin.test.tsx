// UsersAdmin (admin-console scope): create a user, disable it (through ConfirmDestructive, reversible),
// delete it (type-to-confirm, irreversible + grant-revocation consequence). Driven through the real
// hook + api + the contract-identical admin fake. Mirrors MembersView.test.tsx.

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { UsersAdmin } from "./UsersAdmin";
import { setSession } from "@/lib/session/session.store";
import { ADMIN_CAPS } from "@/lib/session/admin-caps";
import { __resetAdminFake } from "@/lib/ipc/admin.fake";

function signIn(workspace: string): void {
  setSession({ token: `t:${workspace}`, principal: "user:ada", workspace, caps: ADMIN_CAPS });
}

beforeEach(() => __resetAdminFake());
afterEach(() => __resetAdminFake());

describe("UsersAdmin", () => {
  it("creates a user and shows it active", async () => {
    const user = userEvent.setup();
    signIn("acme");
    render(<UsersAdmin ws="acme" />);

    await user.type(screen.getByLabelText("new user"), "user:bob");
    await user.click(screen.getByLabelText("create user"));

    expect(await screen.findByText("user:bob")).toBeInTheDocument();
    expect(screen.getByText("active")).toBeInTheDocument();
  });

  it("disable routes through a reversible confirm and flips the status", async () => {
    const user = userEvent.setup();
    signIn("acme");
    render(<UsersAdmin ws="acme" />);
    await user.type(screen.getByLabelText("new user"), "user:bob");
    await user.click(screen.getByLabelText("create user"));
    await screen.findByText("user:bob");

    await user.click(screen.getByLabelText("disable user:bob"));
    // The confirm blocks until confirmed; it is reversible.
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByText("reversible")).toBeInTheDocument();
    await user.click(screen.getByLabelText("confirm action"));

    expect(await screen.findByText("disabled")).toBeInTheDocument();
  });

  it("delete requires typing the user name (irreversible) and removes the row", async () => {
    const user = userEvent.setup();
    signIn("acme");
    render(<UsersAdmin ws="acme" />);
    await user.type(screen.getByLabelText("new user"), "user:bob");
    await user.click(screen.getByLabelText("create user"));
    await screen.findByText("user:bob");

    await user.click(screen.getByLabelText("delete user:bob"));
    expect(screen.getByText("irreversible")).toBeInTheDocument();
    // grant-revocation consequence shown
    expect(screen.getByTestId("consequence")).toHaveTextContent(/revokes ALL their grants/i);
    // blocked until the exact name is typed
    expect(screen.getByLabelText("confirm action")).toBeDisabled();
    await user.type(screen.getByLabelText("type to confirm"), "user:bob");
    await user.click(screen.getByLabelText("confirm action"));

    await screen.findByText("No users yet.");
    expect(screen.queryByText("user:bob")).not.toBeInTheDocument();
  });

  it("cancel performs nothing", async () => {
    const user = userEvent.setup();
    signIn("acme");
    render(<UsersAdmin ws="acme" />);
    await user.type(screen.getByLabelText("new user"), "user:bob");
    await user.click(screen.getByLabelText("create user"));
    await screen.findByText("user:bob");

    await user.click(screen.getByLabelText("delete user:bob"));
    await user.click(screen.getByLabelText("cancel"));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByText("user:bob")).toBeInTheDocument();
  });
});
