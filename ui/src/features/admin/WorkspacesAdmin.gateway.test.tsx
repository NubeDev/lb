// WorkspacesAdmin (admin-console scope), driven against a REAL spawned gateway (CLAUDE §9 — no fake).
// archive (reversible, single confirm) and purge (hard-delete, type-the-name escalation; the backend
// also requires the purge cap + a confirm token == the id). The directory is node-level, so each test
// registers a UNIQUELY-named workspace via the real `createWorkspace`, drives the confirm dialog, and
// asserts the row disappears from the real `workspace_list` (both archive and purge hide the record).
//
// Note: the old fake-based test asserted the fake's internal `__workspaceState(ws)` value. Against the
// real gateway there is no peek into host state, so the observable assertion is the list view: an
// archived/purged workspace is excluded from `workspace_list` (lb_host::workspace_list retains only
// Active; purge writes a tombstone), so its row leaves the table. The wrong-name-stays-blocked gate is
// asserted on the dialog itself (the confirm button stays disabled).

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { WorkspacesAdmin } from "./WorkspacesAdmin";
import { createWorkspace } from "@/lib/workspace/workspace.api";
import { CAP } from "@/lib/session";
import { useRealGateway, signInReal } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `wsadmin-${n++}`;
const nextTarget = () => `pilot-${n++}`;

beforeAll(() => useRealGateway());

describe("WorkspacesAdmin (real gateway)", () => {
  it("archive is reversible and a single confirm — the workspace leaves the active list", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    const target = nextTarget();
    await signInReal("user:ada", ws);
    await createWorkspace(target, "Pilot");

    render(<WorkspacesAdmin ws={ws} />);
    await screen.findByText(target);

    await user.click(screen.getByLabelText(`archive ${target}`));
    expect(screen.getByText("reversible")).toBeInTheDocument();
    await user.click(screen.getByLabelText("confirm action"));

    // After the reversible archive the workspace is hidden from the default (active) directory view.
    await waitFor(() => expect(screen.queryByText(target)).not.toBeInTheDocument());
  });

  it("creates a workspace from the New-workspace form — the new row lands in the real directory", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    const target = nextTarget();
    await signInReal("user:ada", ws);

    // The create control is cap-gated for display; pass the real create cap (the gateway also re-checks).
    render(<WorkspacesAdmin ws={ws} caps={[CAP.workspaceCreate]} />);

    await user.click(screen.getByLabelText("new workspace"));
    await user.type(screen.getByLabelText("workspace id"), target);
    await user.type(screen.getByLabelText("workspace name"), "Pilot");
    await user.click(screen.getByLabelText("create workspace"));

    // The real `workspace_create` writes the directory record, so the row appears in the live list.
    await screen.findByText(target);
  });

  it("purge requires typing the workspace id (the type-name gate) and then tombstones it", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    const target = nextTarget();
    await signInReal("user:ada", ws);
    await createWorkspace(target, "Pilot");

    render(<WorkspacesAdmin ws={ws} />);
    await screen.findByText(target);

    await user.click(screen.getByLabelText(`purge ${target}`));
    expect(screen.getByText("irreversible")).toBeInTheDocument();
    expect(screen.getByLabelText("confirm action")).toBeDisabled();

    // Wrong name keeps it blocked.
    await user.type(screen.getByLabelText("type to confirm"), "wrong");
    expect(screen.getByLabelText("confirm action")).toBeDisabled();

    await user.clear(screen.getByLabelText("type to confirm"));
    await user.type(screen.getByLabelText("type to confirm"), target);
    await user.click(screen.getByLabelText("confirm action"));

    // The hard delete tombstones the directory record, so it leaves the list (and cannot be re-added).
    await waitFor(() => expect(screen.queryByText(target)).not.toBeInTheDocument());
  });
});
