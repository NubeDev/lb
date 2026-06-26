// The S7 registry gates, at the UI level: installing a SIGNED version succeeds and becomes the live
// install; installing one that FAILS verification is refused ("artifact failed verification");
// rolling back to a prior signed version flips the install; and a user without the registry grant is
// denied — the same gates the Rust registry tests prove on the backend, surfaced through the real api
// client + the faithful in-memory fake.
//
// We drive the actual `registry.api` → `invoke` → fake path (no mock of the api): the fake mirrors
// the node's capability + signature gates, so this exercises the allow/deny/unverified/rollback
// branches the user actually hits.

import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { RegistryView } from "./RegistryView";
import {
  __resetRegistryFake,
  __seedCatalog,
  __installedVersion,
} from "@/lib/ipc/registry.fake";

const WS = "acme";
const LIST = "mcp:registry.list:call";
const INSTALL = "mcp:registry.install:call";

function seedTrustedV1V2() {
  __seedCatalog(WS, {
    extId: "hello",
    version: "0.1.0",
    digestHex: "aa",
    publisherKeyId: "pub-1",
    visibility: "private",
    trusted: true,
  });
  __seedCatalog(WS, {
    extId: "hello",
    version: "0.2.0",
    digestHex: "bb",
    publisherKeyId: "pub-1",
    visibility: "private",
    trusted: true,
  });
}

beforeEach(() => __resetRegistryFake());
afterEach(() => __resetRegistryFake());

describe("RegistryView install + verification gates", () => {
  it("installs a signed version — it becomes the live install", async () => {
    seedTrustedV1V2();
    render(<RegistryView ws={WS} extId="hello" author="user:ada" caps={[LIST, INSTALL]} />);

    await screen.findByText("hello@0.2.0");
    await userEvent.click(screen.getByRole("button", { name: /Install 0.2.0/ }));
    await waitFor(() =>
      expect(screen.getByRole("status")).toHaveTextContent("Installed hello@0.2.0"),
    );
    expect(__installedVersion(WS, "hello")).toBe("0.2.0");
  });

  it("refuses a version whose artifact fails verification — nothing installed", async () => {
    seedTrustedV1V2();
    __seedCatalog(WS, {
      extId: "hello",
      version: "0.3.0",
      digestHex: "cc",
      publisherKeyId: "evil",
      visibility: "private",
      trusted: false, // tampered / unsigned / untrusted key
    });
    render(<RegistryView ws={WS} extId="hello" author="user:ada" caps={[LIST, INSTALL]} />);

    await screen.findByText("hello@0.3.0");
    await userEvent.click(screen.getByRole("button", { name: /Install 0.3.0/ }));
    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent("Artifact failed verification"),
    );
    expect(__installedVersion(WS, "hello")).toBeUndefined();
  });

  it("rolls back to a prior signed version — the same install verb, a prior version", async () => {
    seedTrustedV1V2();
    render(<RegistryView ws={WS} extId="hello" author="user:ada" caps={[LIST, INSTALL]} />);

    await screen.findByText("hello@0.2.0");
    await userEvent.click(screen.getByRole("button", { name: /Install 0.2.0/ }));
    await waitFor(() => expect(__installedVersion(WS, "hello")).toBe("0.2.0"));

    // Now 0.1.0's button reads "Roll back to 0.1.0" — rollback is the same verb on a prior version.
    await userEvent.click(screen.getByRole("button", { name: /Roll back to 0.1.0/ }));
    await waitFor(() => expect(__installedVersion(WS, "hello")).toBe("0.1.0"));
    expect(screen.getByRole("status")).toHaveTextContent("Installed hello@0.1.0");
  });

  it("a user WITHOUT the registry grant is denied", async () => {
    seedTrustedV1V2();
    render(<RegistryView ws={WS} extId="hello" author="user:cleo" caps={[]} />);
    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(
        "You don't have access to the registry.",
      ),
    );
  });
});
