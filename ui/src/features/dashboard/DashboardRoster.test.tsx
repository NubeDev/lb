// The dashboard roster — unit coverage for the create / rename / delete affordances (dashboard scope).
// Pure component render (no gateway): the roster owns markup + local edit state; the caller owns the
// actual `dashboard.*` verb calls, which we assert are invoked with the right args. Rename/delete are
// gated on `canEdit` (the session `dashboard.save` grant) and delete routes through the shared
// `ConfirmDestructive` gate.

import type React from "react";
import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { DashboardRoster } from "./DashboardRoster";
import type { DashboardSummary } from "@/lib/dashboard";

const roster: DashboardSummary[] = [
  { id: "ops", title: "Ops", visibility: "private", updated_ts: 1 },
  { id: "infra", title: "Infra", visibility: "workspace", updated_ts: 1 },
];

function renderRoster(overrides: Partial<React.ComponentProps<typeof DashboardRoster>> = {}) {
  const props = {
    roster,
    selectedId: null,
    onSelect: vi.fn(),
    onCreate: vi.fn(),
    onRename: vi.fn(),
    onRemove: vi.fn(),
    canEdit: true,
    ...overrides,
  };
  render(<DashboardRoster {...props} />);
  return props;
}

describe("DashboardRoster", () => {
  it("creates a dashboard with a slugified id from the typed title", async () => {
    const user = userEvent.setup();
    const props = renderRoster();
    await user.type(screen.getByLabelText("new dashboard title"), "My New Board");
    await user.click(screen.getByLabelText("create dashboard"));
    expect(props.onCreate).toHaveBeenCalledWith("my-new-board", "My New Board");
  });

  it("renames a dashboard inline (pencil → edit → confirm) with the new title", async () => {
    const user = userEvent.setup();
    const props = renderRoster();
    await user.click(screen.getByLabelText("rename dashboard ops"));
    const field = screen.getByLabelText("rename dashboard ops");
    await user.clear(field);
    await user.type(field, "Operations");
    await user.click(screen.getByLabelText("confirm rename ops"));
    expect(props.onRename).toHaveBeenCalledWith("ops", "Operations");
  });

  it("cancels a rename without calling onRename", async () => {
    const user = userEvent.setup();
    const props = renderRoster();
    await user.click(screen.getByLabelText("rename dashboard ops"));
    await user.type(screen.getByLabelText("rename dashboard ops"), "X");
    await user.click(screen.getByLabelText("cancel rename ops"));
    expect(props.onRename).not.toHaveBeenCalled();
  });

  it("deletes a dashboard only after the destructive confirm", async () => {
    const user = userEvent.setup();
    const props = renderRoster();
    await user.click(screen.getByLabelText("delete dashboard ops"));
    // Not removed until the confirm dialog's Delete is clicked.
    expect(props.onRemove).not.toHaveBeenCalled();
    await user.click(screen.getByLabelText("confirm action"));
    expect(props.onRemove).toHaveBeenCalledWith("ops");
  });

  it("hides rename/delete controls when the caller cannot edit", () => {
    renderRoster({ canEdit: false });
    expect(screen.queryByLabelText("rename dashboard ops")).toBeNull();
    expect(screen.queryByLabelText("delete dashboard ops")).toBeNull();
  });
});
