import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { WidgetLiveTile } from "@/app/WidgetLiveTile";
import { watchBridge, rejectingBridge } from "@/test/bridge.stub";

// The LIVE (SSE) widget tile, against the v2 bridge INTERFACE the shell provides (test double, not a
// fake node — testing-scope §0). Proves the motion contract: backfill once via `series.latest`, then
// update per live `series.watch` sample, then tear the stream down on unmount. The live path over a
// REAL gateway is asserted end to end by the dashboard-widget Playwright e2e.

describe("WidgetLiveTile (SSE)", () => {
  it("backfills the latest value, then updates on each live sample", async () => {
    const { bridge, emit } = watchBridge({
      "series.latest": () => ({ sample: { payload: 7, seq: 1 } }),
    });

    render(<WidgetLiveTile bridge={bridge} />);

    // 1) Backfill: the one-shot `series.latest` value renders, and the tile is "idle" (no sample yet).
    expect(await screen.findByLabelText("proof live widget value")).toHaveTextContent("7");
    expect(screen.getByText("idle")).toBeInTheDocument();

    // 2) A live sample arrives over `series.watch` → the value updates and the badge flips to "live".
    emit({ payload: 42, seq: 2 });
    await waitFor(() =>
      expect(screen.getByLabelText("proof live widget value")).toHaveTextContent("42"),
    );
    expect(screen.getByText("live")).toBeInTheDocument();

    // 3) A second live sample folds in with no reload.
    emit({ payload: 99, seq: 3 });
    await waitFor(() =>
      expect(screen.getByLabelText("proof live widget value")).toHaveTextContent("99"),
    );
  });

  it("tears the stream down on unmount (stateless eviction)", async () => {
    const { bridge, unsubscribed } = watchBridge({
      "series.latest": () => ({ sample: { payload: 1, seq: 1 } }),
    });
    const { unmount } = render(<WidgetLiveTile bridge={bridge} />);
    await screen.findByLabelText("proof live widget value");
    expect(unsubscribed()).toBe(false);
    unmount();
    expect(unsubscribed()).toBe(true);
  });

  it("shows an honest 'no access' state when the backfill read is denied", async () => {
    render(<WidgetLiveTile bridge={rejectingBridge("denied")} />);
    expect(await screen.findByText("no access")).toBeInTheDocument();
    expect(screen.queryByLabelText("proof live widget value")).not.toBeInTheDocument();
  });
});
