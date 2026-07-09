// Unit coverage for the reminders stat strip — the KPIs are derived purely from the record list
// (`reminder.list`), so this needs no gateway: seed plain `Reminder` objects and assert the tiles.
// Proves the honest record-level facts (counts by state, summed firings) render, and that a paused
// or done reminder is bucketed correctly (not counted as active).

import { describe, expect, it } from "vitest";
import { render, screen, within } from "@testing-library/react";

import { RemindersStats } from "./RemindersStats";
import type { Reminder } from "@/lib/reminders/reminders.types";

function reminder(over: Partial<Reminder>): Reminder {
  return {
    id: "r",
    schedule: "0 8 * * *",
    maxRuns: null,
    runs: 0,
    enabled: true,
    status: "active",
    action: { kind: "channel-post", channel: "team", body: "hi" },
    principalSub: "user:ada",
    nextAttemptTs: 0,
    ts: 0,
    ...over,
  };
}

/** Read a stat tile's value — the `.text-lg` value line inside the tile carrying `label`. */
function tile(label: string): string {
  const strip = screen.getByLabelText("reminder stats");
  const card = within(strip).getByText(label).closest("div")!.parentElement!;
  return card.querySelector(".text-lg")?.textContent ?? "";
}

describe("RemindersStats", () => {
  it("buckets reminders by state and sums firings", () => {
    render(
      <RemindersStats
        reminders={[
          reminder({ id: "a", enabled: true, status: "active", runs: 3 }),
          reminder({ id: "b", enabled: false, status: "active", runs: 5 }),
          reminder({ id: "c", enabled: false, status: "done", runs: 10 }),
        ]}
      />,
    );

    expect(tile("Reminders")).toBe("3");
    expect(tile("Active")).toBe("1");
    expect(tile("Paused")).toBe("1");
    expect(tile("Completed")).toBe("1");
    expect(tile("Total firings")).toBe("18"); // 3 + 5 + 10
  });

  it("shows an em dash for next firing when nothing is scheduled", () => {
    render(<RemindersStats reminders={[reminder({ enabled: false, nextAttemptTs: 0 })]} />);
    const strip = screen.getByLabelText("reminder stats");
    expect(within(strip).getByText("—")).toBeInTheDocument();
  });
});
