// GrantsAdmin (admin-console scope): read a subject's grants; assign a cap; revoke one through a
// reversible confirm. Read + assign/revoke only — there is NO role editor this slice. Driven through
// the real hook + api + the admin fake.

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { GrantsAdmin } from "./GrantsAdmin";
import { setSession } from "@/lib/session/session.store";
import { ADMIN_CAPS } from "@/lib/session/admin-caps";
import { __resetAdminFake } from "@/lib/ipc/admin.fake";

function signIn(workspace: string): void {
  setSession({ token: `t:${workspace}`, principal: "user:ada", workspace, caps: ADMIN_CAPS });
}

beforeEach(() => __resetAdminFake());
afterEach(() => __resetAdminFake());

describe("GrantsAdmin", () => {
  it("assigns a cap and then revokes it through a reversible confirm", async () => {
    const user = userEvent.setup();
    signIn("acme");
    render(<GrantsAdmin ws="acme" />);
    await screen.findByText(/no grants for user:bob/i);

    await user.type(screen.getByLabelText("cap to assign"), "mcp:inbox.list:call");
    await user.click(screen.getByLabelText("assign"));
    expect(await screen.findByText("mcp:inbox.list:call")).toBeInTheDocument();

    await user.click(screen.getByLabelText("revoke mcp:inbox.list:call"));
    expect(screen.getByText("reversible")).toBeInTheDocument();
    await user.click(screen.getByLabelText("confirm action"));

    expect(await screen.findByText(/no grants for user:bob/i)).toBeInTheDocument();
  });
});
