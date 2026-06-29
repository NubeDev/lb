// api-keys scope — the API Keys admin tab over a REAL spawned gateway (CLAUDE §9 — no fake). The raw
// secret a create returns is shown EXACTLY ONCE (a banner) then dismissed; the list NEVER renders a
// hash or secret; revoke flips a row to "revoked". The cap-gate (tab hidden without `apikey.manage`)
// is asserted in AdminView.gateway.test.tsx. Each test signs into a UNIQUE real workspace.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ApiKeysAdmin } from "./ApiKeysAdmin";
import { useRealGateway, signInReal } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `apikeys-${n++}`;

beforeAll(() => useRealGateway());

describe("ApiKeysAdmin (real gateway)", () => {
  it("create shows the secret once, then dismisses it; the list never renders the secret", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    render(<ApiKeysAdmin ws={ws} />);

    await user.click(await screen.findByRole("button", { name: "new key" }));
    await user.type(screen.getByLabelText("key label"), "rooftop-hvac");
    await user.click(screen.getByLabelText("create key"));

    // The one-time secret banner appears with the bearer + the warning.
    const banner = await screen.findByRole("alert");
    const secret = within(banner).getByText(/lbk_/).textContent!;
    expect(secret).toContain(`${ws}.`);
    expect(banner.textContent).toMatch(/won't see this secret again/i);

    // Dismiss it.
    await user.click(screen.getByLabelText("dismiss secret"));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();

    // The list shows the label but NEVER the secret.
    const table = await screen.findByRole("table");
    expect(await within(table).findByText("rooftop-hvac")).toBeInTheDocument();
    expect(table.textContent).not.toContain(secret);
    expect(table.textContent).not.toContain("key_hash");
  });

  it("revoke flips the row to revoked and hides the revoke/rotate actions", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    render(<ApiKeysAdmin ws={ws} />);

    // Create a key first (dismiss the secret banner so only the table remains).
    await user.click(await screen.findByRole("button", { name: "new key" }));
    await user.type(screen.getByLabelText("key label"), "to-revoke");
    await user.click(screen.getByLabelText("create key"));
    await screen.findByRole("alert");
    await user.click(screen.getByLabelText("dismiss secret"));

    const revokeBtn = await screen.findByRole("button", { name: /revoke key / });
    await user.click(revokeBtn);

    // The row flips to revoked and the destructive actions are gone.
    const table = await screen.findByRole("table");
    expect(await within(table).findByText("revoked")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /revoke key / })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /rotate key / })).not.toBeInTheDocument();
  });
});
