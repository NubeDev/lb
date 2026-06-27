// The shared destructive-confirm UX (admin-console scope): blocks until an explicit, satisfied
// confirm; shows reversible vs irreversible; escalates to type-to-confirm for hard-delete; a second
// gate for the checkbox path; and Cancel performs nothing. Every admin delete routes through this, so
// these branches are the whole safety story — tested directly.

import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ConfirmDestructive } from "./ConfirmDestructive";

describe("ConfirmDestructive", () => {
  it("single confirm: reversible, one click runs the action", async () => {
    const user = userEvent.setup();
    const onConfirm = vi.fn();
    render(
      <ConfirmDestructive
        title="Disable bob"
        consequence="bob can't sign in until re-enabled."
        reversible
        escalation="none"
        onConfirm={onConfirm}
        onCancel={vi.fn()}
      />,
    );
    expect(screen.getByText("reversible")).toBeInTheDocument();
    expect(screen.getByText(/can't sign in until re-enabled/)).toBeInTheDocument();
    await user.click(screen.getByLabelText("confirm action"));
    expect(onConfirm).toHaveBeenCalledOnce();
  });

  it("cancel performs nothing", async () => {
    const user = userEvent.setup();
    const onConfirm = vi.fn();
    const onCancel = vi.fn();
    render(
      <ConfirmDestructive
        title="Delete team"
        consequence="2 members, removes their inherited caps."
        reversible={false}
        escalation="none"
        onConfirm={onConfirm}
        onCancel={onCancel}
      />,
    );
    await user.click(screen.getByLabelText("cancel"));
    expect(onCancel).toHaveBeenCalledOnce();
    expect(onConfirm).not.toHaveBeenCalled();
  });

  it("type-name: irreversible, Confirm blocked until the exact name is typed", async () => {
    const user = userEvent.setup();
    const onConfirm = vi.fn();
    render(
      <ConfirmDestructive
        title="Purge workspace pilot"
        consequence="all data destroyed, tombstoned, unrecoverable."
        reversible={false}
        escalation="type-name"
        confirmName="pilot"
        confirmLabel="Purge"
        onConfirm={onConfirm}
        onCancel={vi.fn()}
      />,
    );
    expect(screen.getByText("irreversible")).toBeInTheDocument();
    const confirm = screen.getByLabelText("confirm action");
    expect(confirm).toBeDisabled();

    await user.type(screen.getByLabelText("type to confirm"), "wrong");
    expect(confirm).toBeDisabled();
    await user.click(confirm);
    expect(onConfirm).not.toHaveBeenCalled();

    await user.clear(screen.getByLabelText("type to confirm"));
    await user.type(screen.getByLabelText("type to confirm"), "pilot");
    expect(confirm).toBeEnabled();
    await user.click(confirm);
    expect(onConfirm).toHaveBeenCalledOnce();
  });

  it("second-gate: Confirm blocked until the checkbox is acknowledged", async () => {
    const user = userEvent.setup();
    const onConfirm = vi.fn();
    render(
      <ConfirmDestructive
        title="Uninstall echo-sidecar"
        consequence="removes the install record and cached binary."
        reversible={false}
        escalation="second-gate"
        onConfirm={onConfirm}
        onCancel={vi.fn()}
      />,
    );
    const confirm = screen.getByLabelText("confirm action");
    expect(confirm).toBeDisabled();
    await user.click(screen.getByLabelText("acknowledge"));
    expect(confirm).toBeEnabled();
    await user.click(confirm);
    expect(onConfirm).toHaveBeenCalledOnce();
  });
});
