// The Datasources admin page, driven against a REAL in-process gateway (rules-workbench scope, Phase 3;
// CLAUDE §9 / testing §0 — no fake backend, no `*.fake.ts`). Each test logs in to a UNIQUE workspace and
// drives the real `DatasourcesAdmin` + hook + api client + HTTP transport against the live `datasource.*`
// host verbs. Covers: empty roster in a fresh ws; add → list shows it (kind + endpoint + redacted secret
// ref) and the response NEVER contains the submitted DSN (REDACTION); the add form shows the implied
// grants; remove drops it; and the probe renders an HONEST RED (the federation sidecar is not installed
// in this test env, so `datasource.test` fails with a real typed error — never a fabricated green).

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { DatasourcesAdmin } from "./DatasourcesAdmin";
import { useRealGateway, signInReal } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `ds-ui-${n++}`;

beforeAll(() => useRealGateway());

/** The DSN we submit — secret material that must NEVER appear in any rendered roster. */
const DSN = "host=127.0.0.1 port=5432 user=lb password=UISECRETpw dbname=fed sslmode=disable";

async function addSource(
  user: ReturnType<typeof userEvent.setup>,
  fields: { name: string; kind: string; endpoint: string; dsn: string },
) {
  await user.type(await screen.findByLabelText("datasource name"), fields.name);
  const kind = screen.getByLabelText("datasource kind");
  await user.clear(kind);
  await user.type(kind, fields.kind);
  await user.type(screen.getByLabelText("datasource endpoint"), fields.endpoint);
  await user.type(screen.getByLabelText("datasource dsn"), fields.dsn);
  await user.click(screen.getByLabelText("submit datasource"));
}

describe("DatasourcesAdmin (real gateway)", () => {
  it("a fresh workspace shows an empty roster", async () => {
    const ws = nextWs();
    await signInReal("user:ada", ws);
    render(<DatasourcesAdmin ws={ws} onOpen={() => {}} />);
    expect(await screen.findByText("No datasources yet.")).toBeInTheDocument();
  });

  it("adds a datasource, lists it, and never renders the DSN (redaction)", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    const { container } = render(<DatasourcesAdmin ws={ws} onOpen={() => {}} />);
    await screen.findByText("No datasources yet.");

    await addSource(user, {
      name: "timescale",
      kind: "postgres",
      endpoint: "tsdb.acme:5432",
      dsn: DSN,
    });

    // The roster row renders kind + endpoint + a redacted secret ref read back through the real gateway.
    const row = within(await screen.findByLabelText("datasource timescale"));
    expect(row.getByText("postgres")).toBeInTheDocument();
    expect(row.getByText("tsdb.acme:5432")).toBeInTheDocument();
    expect(row.getByLabelText("secret ref timescale").textContent).toContain("federation/timescale");

    // REDACTION: the DSN (and its password) appears NOWHERE in the rendered page.
    expect(container.textContent).not.toContain(DSN);
    expect(container.textContent).not.toContain("UISECRETpw");
  });

  it("shows the implied grants derived from the form", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    render(<DatasourcesAdmin ws={ws} onOpen={() => {}} />);

    await user.type(await screen.findByLabelText("datasource name"), "timescale");
    await user.type(screen.getByLabelText("datasource endpoint"), "tsdb.acme:5432");

    const grants = within(await screen.findByLabelText("implied grants"));
    expect(grants.getByText("net:tls:tsdb.acme:5432:connect")).toBeInTheDocument();
    expect(grants.getByText("secret:federation/timescale:get")).toBeInTheDocument();
  });

  it("removes a datasource", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    render(<DatasourcesAdmin ws={ws} onOpen={() => {}} />);
    await screen.findByText("No datasources yet.");

    await addSource(user, { name: "ts", kind: "postgres", endpoint: "tsdb.acme:5432", dsn: DSN });
    await screen.findByLabelText("datasource ts");

    await user.click(screen.getByLabelText("remove ts"));
    expect(await screen.findByText("No datasources yet.")).toBeInTheDocument();
  });

  it("the probe renders an HONEST RED with no sidecar installed (never a fabricated green)", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signInReal("user:ada", ws);
    render(<DatasourcesAdmin ws={ws} onOpen={() => {}} />);
    await screen.findByText("No datasources yet.");

    await addSource(user, { name: "ts", kind: "postgres", endpoint: "tsdb.acme:5432", dsn: DSN });
    await screen.findByLabelText("datasource ts");

    // No federation sidecar in this env → `datasource.test` fails with a real typed error → RED.
    await user.click(screen.getByLabelText("test ts"));
    const badge = await screen.findByLabelText("probe ts");
    expect(badge).toHaveAttribute("data-state", "red");
    expect(badge.textContent).toMatch(/Failed/);
  });
});
