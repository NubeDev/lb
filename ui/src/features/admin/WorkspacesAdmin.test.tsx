// WorkspacesAdmin (admin-console scope): archive (reversible, single confirm) and purge (hard-delete,
// type-the-name escalation; the backend also requires the purge cap + a confirm token == the id). The
// fake's `workspace_purge` rejects a mismatched confirm, mirroring the host's typed-confirm gate.
// Driven through the real hook + api + the workspace/admin fakes.

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { WorkspacesAdmin } from "./WorkspacesAdmin";
import { setSession } from "@/lib/session/session.store";
import { ADMIN_CAPS } from "@/lib/session/admin-caps";
import { __resetAdminFake, __workspaceState } from "@/lib/ipc/admin.fake";
import { __resetWorkspaceFake, workspaceFakeInvoke } from "@/lib/ipc/workspace.fake";

function signIn(workspace: string): void {
  setSession({ token: `t:${workspace}`, principal: "user:ada", workspace, caps: ADMIN_CAPS });
}

beforeEach(() => {
  __resetAdminFake();
  __resetWorkspaceFake();
});
afterEach(() => {
  __resetAdminFake();
  __resetWorkspaceFake();
});

describe("WorkspacesAdmin", () => {
  it("archive is reversible and a single confirm", async () => {
    const user = userEvent.setup();
    signIn("acme");
    workspaceFakeInvoke("workspace_create", { ws: "pilot", name: "Pilot" });
    render(<WorkspacesAdmin ws="acme" />);
    await screen.findByText("pilot");

    await user.click(screen.getByLabelText("archive pilot"));
    expect(screen.getByText("reversible")).toBeInTheDocument();
    await user.click(screen.getByLabelText("confirm action"));

    expect(__workspaceState("pilot")).toBe("archived");
  });

  it("purge requires typing the workspace id (the type-name gate) and then tombstones it", async () => {
    const user = userEvent.setup();
    signIn("acme");
    workspaceFakeInvoke("workspace_create", { ws: "pilot", name: "Pilot" });
    render(<WorkspacesAdmin ws="acme" />);
    await screen.findByText("pilot");

    await user.click(screen.getByLabelText("purge pilot"));
    expect(screen.getByText("irreversible")).toBeInTheDocument();
    expect(screen.getByLabelText("confirm action")).toBeDisabled();

    // Wrong name keeps it blocked.
    await user.type(screen.getByLabelText("type to confirm"), "wrong");
    expect(screen.getByLabelText("confirm action")).toBeDisabled();

    await user.clear(screen.getByLabelText("type to confirm"));
    await user.type(screen.getByLabelText("type to confirm"), "pilot");
    await user.click(screen.getByLabelText("confirm action"));

    expect(__workspaceState("pilot")).toBe("purged");
  });
});
