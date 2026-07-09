// The Ingest wizard, driven against a REAL in-process gateway (setup scope; CLAUDE §9 — no fake
// backend). Proves the reported gap is closed: quick-creating a series in step 1 PERSISTS it (a real
// record readable back over the gateway), Continue is blocked until it does, step 2 mints a real API
// key, and step 3's Python snippet is prefilled with the workspace, series, and the minted `lbk_…`
// key. A fresh workspace per test isolates the shared real node.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { IngestWizard } from "./IngestWizard";
import { loadSchema } from "@/lib/ingest/schema.api";
import { CAP } from "@/lib/session/admin-caps";
import { useRealGateway, signInReal } from "@/test/gateway-session";

const CAPS = [CAP.ingestWrite, CAP.apikeyManage];

let n = 0;
const nextWs = () => `ingest-wiz-${n++}`;

beforeAll(() => useRealGateway());

describe("IngestWizard (real gateway)", () => {
  it("actually creates the series, mints a key, and prefills the Python snippet", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    render(<IngestWizard ws={ws} caps={CAPS} />);

    // ── Step 1: Continue is blocked until the series is really created. ──
    await user.type(await screen.findByLabelText("Series name"), "demo.cpu_temp");
    expect(screen.getByLabelText("Continue")).toBeDisabled();

    await user.click(screen.getByLabelText("Create series"));

    // The series persisted as a real, typed record — read its schema back over the real gateway.
    // (A series is defined by its schema record here; a producer's first sample fills in data. This is
    // the SAME create path the ingest explorer's own CreateSeriesWizard uses.)
    await waitFor(async () => {
      const s = await loadSchema("demo.cpu_temp");
      expect(s).not.toBeNull();
      expect(s!.fields.length).toBeGreaterThan(0);
    });

    // Now the confirmation shows and Continue enables.
    await screen.findByText(/ready to receive samples/i);
    await waitFor(() => expect(screen.getByLabelText("Continue")).toBeEnabled());
    await user.click(screen.getByLabelText("Continue"));

    // ── Step 2: mint a real key; the one-time bearer shows. ──
    await user.click(await screen.findByLabelText("Mint an API key"));
    const banner = await screen.findByRole("alert");
    const bearer = within(banner).getByText(/^lbk_/);
    expect(bearer.textContent).toMatch(/^lbk_/);
    await user.click(screen.getByLabelText("Continue"));

    // ── Step 3: the snippet is prefilled with ws + series + the minted key. ──
    const code = (await screen.findByText(/from lb_client import/i)).textContent ?? "";
    expect(code).toContain('"demo.cpu_temp"');
    expect(code).toContain(bearer.textContent ?? "lbk_");
    expect(code).not.toContain("lbk_REPLACE_WITH_YOUR_API_KEY");
  });
});
