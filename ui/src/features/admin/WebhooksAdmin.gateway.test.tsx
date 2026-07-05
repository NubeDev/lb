// webhooks scope — the Webhooks admin tab over a REAL spawned gateway (CLAUDE §9 — no fake). The
// raw secret a create returns is shown EXACTLY ONCE (a banner with the URL + the secret + the
// mode-specific "how to call" copy) then dismissed; the list NEVER renders a hash, secret,
// `bearer_key_id`, or `secret_ref`. The mode picker switches the create form between `bearer`
// (no header field) and `signature` (header field). Revoke flips a row to "revoked" and hides the
// rotate/revoke actions. Rotate re-arms the one-time banner with a fresh secret. Each test signs
// into a UNIQUE real workspace.

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { WebhooksAdmin } from "./WebhooksAdmin";
import { useRealGateway, signInReal } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `webhooks-${n++}`;

beforeAll(() => useRealGateway());

describe("WebhooksAdmin (real gateway)", () => {
  it("creates a signature-mode webhook and shows the URL + shared secret once, then dismisses", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    render(<WebhooksAdmin ws={ws} />);

    await user.click(await screen.findByRole("button", { name: "new webhook" }));
    await user.type(screen.getByLabelText("webhook name"), "plant-alerts");
    // Default mode is `signature` → the header field is visible.
    expect(screen.getByLabelText("hmac header")).toBeInTheDocument();
    await user.click(screen.getByLabelText("create webhook"));

    // The one-time banner appears with the URL + the shared secret + the header-name hint.
    const banner = await screen.findByRole("alert");
    expect(within(banner).getByText(/Inbound URL/i)).toBeInTheDocument();
    expect(within(banner).getByText(/HMAC-SHA256-sign the raw body/i)).toBeInTheDocument();
    expect(banner.textContent).toMatch(/X-Signature/i);
    expect(banner.textContent).toMatch(/won't see this again/i);

    // The shared secret is captured for the leak assertion below, then dismissed.
    const secret = within(banner)
      .getAllByText(/\b[A-Z0-9]{20,}\b/i)
      .map((n) => n.textContent ?? "")
      .find((s) => s.length >= 40 && !s.includes(" "))!;
    expect(secret).toBeTruthy();

    await user.click(screen.getByLabelText("dismiss secret"));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();

    // The list shows the name + URL; NEVER the secret/hash/secret_ref/bearer_key_id.
    const table = await screen.findByRole("table");
    expect(await within(table).findByText("plant-alerts")).toBeInTheDocument();
    expect(table.textContent).not.toContain(secret);
    expect(table.textContent).not.toContain("secret");
    expect(table.textContent).not.toContain("hash");
    expect(table.textContent).not.toContain("bearer_key_id");
    expect(table.textContent).not.toContain("secret_ref");
  });

  it("creates a bearer-mode webhook and shows the lbk_ credential + the Authorization hint", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    render(<WebhooksAdmin ws={ws} />);

    await user.click(await screen.findByRole("button", { name: "new webhook" }));
    await user.type(screen.getByLabelText("webhook name"), "home-automation");
    // Switch to bearer mode — the header field disappears (no HMAC header for bearer).
    await user.click(screen.getByLabelText("mode bearer"));
    expect(screen.queryByLabelText("hmac header")).not.toBeInTheDocument();
    await user.click(screen.getByLabelText("create webhook"));

    const banner = await screen.findByRole("alert");
    expect(banner.textContent).toMatch(/Authorization: Bearer/i);
    const secret = within(banner).getByText(/lbk_/).textContent!;
    expect(secret).toContain(`${ws}.`);
    expect(banner.textContent).toMatch(/won't see this again/i);

    // Dismiss; the list never leaks the bearer.
    await user.click(screen.getByLabelText("dismiss secret"));
    const table = await screen.findByRole("table");
    expect(table.textContent).not.toContain(secret);
  });

  it("rotates a webhook and re-arms the one-time banner with a fresh secret", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    render(<WebhooksAdmin ws={ws} />);

    // Create + dismiss the initial secret banner.
    await user.click(await screen.findByRole("button", { name: "new webhook" }));
    await user.type(screen.getByLabelText("webhook name"), "to-rotate");
    await user.click(screen.getByLabelText("create webhook"));
    const firstBanner = await screen.findByRole("alert");
    const firstSecret = within(firstBanner)
      .getAllByText(/[A-Z0-9]{20,}/i)
      .map((n) => n.textContent ?? "")
      .find((s) => s.length >= 40 && !s.includes(" "))!;
    await user.click(screen.getByLabelText("dismiss secret"));

    // Rotate — a new one-time banner appears with a DIFFERENT secret.
    await user.click(await screen.findByRole("button", { name: /rotate webhook / }));
    const secondBanner = await screen.findByRole("alert");
    const secondSecret = within(secondBanner)
      .getAllByText(/[A-Z0-9]{20,}/i)
      .map((n) => n.textContent ?? "")
      .find((s) => s.length >= 40 && !s.includes(" "))!;
    expect(secondSecret).not.toBe(firstSecret);
    await user.click(screen.getByLabelText("dismiss secret"));
  });

  it("revokes a webhook and hides the rotate/revoke actions on the row", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    render(<WebhooksAdmin ws={ws} />);

    // Create + dismiss.
    await user.click(await screen.findByRole("button", { name: "new webhook" }));
    await user.type(screen.getByLabelText("webhook name"), "to-revoke");
    await user.click(screen.getByLabelText("create webhook"));
    await screen.findByRole("alert");
    await user.click(screen.getByLabelText("dismiss secret"));

    await user.click(await screen.findByRole("button", { name: /revoke webhook / }));

    // Row flips to revoked and the destructive actions are gone.
    const table = await screen.findByRole("table");
    expect(await within(table).findByText("revoked")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /revoke webhook / })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /rotate webhook / })).not.toBeInTheDocument();
  });
});
