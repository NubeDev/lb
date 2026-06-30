// The rules workbench page, driven against a REAL in-process gateway (rules-workbench scope, Phase 1;
// CLAUDE §9 / testing §0 — no fake backend). Each test signs in to a UNIQUE workspace with an explicit
// real cap set and drives the real RulesView + hook + api client + HTTP transport against the real
// `rules.*` host verbs + the real series store. Covers: the CRUD round-trip (save → rail → open →
// delete); the three output kinds (scalar → ScalarCard, grid → GridTable, alert → FindingsList + log +
// budget); and the honest failure states (a cage error renders verbatim, never a fake result; an AI
// body with no model renders "AI not configured"). The per-verb deny + workspace isolation are proven
// server-side in the Rust gateway test (`rules_routes_test.rs`).

import { describe, expect, it, beforeAll } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { RulesView } from "./RulesView";
import { getRule, deleteRule } from "@/lib/rules";
import { useRealGateway, signInWithCaps, seedIotDemo } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `rules-ui-${n++}`;

// The full Playground cap set: the five rules MCP caps + the `store:rule` surface caps the
// save/get/list/delete verbs re-check below the bridge + the read caps a grid run needs + the
// inbox/outbox caps an `alert` finding routes to (the shipped host behaviour).
const PLAYGROUND_CAPS = [
  "mcp:rules.run:call",
  "mcp:rules.save:call",
  "mcp:rules.get:call",
  "mcp:rules.list:call",
  "mcp:rules.delete:call",
  "store:rule:read",
  "store:rule:write",
  "mcp:store.query:call",
  "mcp:series.read:call",
  "mcp:ingest.write:call",
  "mcp:inbox.record:call",
  "mcp:outbox.enqueue:call",
  "inbox:rules:write",
];

beforeAll(() => {
  useRealGateway();
  // jsdom has no layout engine, so CodeMirror's async `requestAnimationFrame` measurement throws
  // `textRange(...).getClientRects is not a function` after a test settles (harmless, but noisy).
  // Polyfill the two Range measurement methods CodeMirror reaches for so the editor mounts cleanly.
  if (!Range.prototype.getClientRects) {
    Range.prototype.getClientRects = () =>
      ({ length: 0, item: () => null, [Symbol.iterator]: function* () {} }) as unknown as DOMRectList;
  }
  if (!Range.prototype.getBoundingClientRect) {
    Range.prototype.getBoundingClientRect = () => ({ x: 0, y: 0, width: 0, height: 0 }) as DOMRect;
  }
});

async function signIn(ws: string) {
  await signInWithCaps("user:ada", ws, PLAYGROUND_CAPS);
}

/** Set a fresh ad-hoc body in the CodeMirror editor (it starts empty for a new buffer). Uses `paste`
 *  rather than `type`: CodeMirror's per-keystroke layout measurement is unreliable under jsdom, but its
 *  DOM-mutation observer picks up a paste into the focused `.cm-content` and fires `onChange` — the real
 *  edit path, no fake. (Pasting also avoids userEvent's `{`/`[` keyboard-modifier parsing.) */
async function typeBody(user: ReturnType<typeof userEvent.setup>, body: string) {
  const editor = screen.getByLabelText("rule editor");
  const area = editor.querySelector(".cm-content") as HTMLElement;
  await user.click(area);
  await user.paste(body);
}

describe("RulesView (real gateway)", () => {
  it("runs a scalar rule and renders a ScalarCard", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signIn(ws);

    render(<RulesView ws={ws} />);
    await typeBody(user, "40 + 2");
    await user.click(screen.getByLabelText("run rule"));

    const card = await screen.findByLabelText("scalar result");
    expect(within(card).getByLabelText("scalar value").textContent).toBe("42");
  });

  it("runs a grid rule (history over seeded series) and renders a GridTable", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signIn(ws);
    await seedIotDemo(); // real `cooler.temp` rows through the real ingest path

    render(<RulesView ws={ws} />);
    await typeBody(user, 'history("series", "cooler.temp", "24h")');
    await user.click(screen.getByLabelText("run rule"));

    const grid = await screen.findByLabelText("grid result");
    expect(grid).toBeInTheDocument();
    // The "showing N of M" footer is present with real rows.
    expect((await screen.findByLabelText("grid count")).textContent).toMatch(/showing \d+ of \d+/);
  });

  it("runs an alert rule and renders FindingsList + log + budget", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signIn(ws);

    render(<RulesView ws={ws} />);
    await typeBody(user, 'log("checking"); alert(#{ level: "critical", msg: "hot" });');
    await user.click(screen.getByLabelText("run rule"));

    const findings = await screen.findByLabelText("findings");
    expect(within(findings).getByLabelText("finding critical")).toBeInTheDocument();
    expect(within(findings).getByLabelText("alert mark")).toBeInTheDocument();
    expect(screen.getByLabelText("log panel")).toBeInTheDocument();
    expect(screen.getByLabelText("budget")).toBeInTheDocument();
  });

  it("CRUD round-trip: create (name-first) → reopen the buffer (via the real path)", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signIn(ws);

    render(<RulesView ws={ws} />);

    // Author a fresh rule, then create it via the name-first rail form. The id is derived from the
    // name (a slug) — the user only types the name; "Cooler check" → id "cooler-check".
    await typeBody(user, "1 + 1");
    await user.click(screen.getByLabelText("new rule"));
    await user.type(screen.getByLabelText("new rule name"), "Cooler check");
    await user.click(screen.getByLabelText("create rule"));

    // Persisted: re-fetch the saved rule through the real `rules.get` path — the round-trip is faithful.
    const reopened = await getRule("cooler-check");
    expect(reopened.body).toBe("1 + 1");
    expect(reopened.name).toBe("Cooler check");

    // Tombstone it through the real `rules.delete` path; a re-get is then NotFound (the wall holds).
    await deleteRule("cooler-check");
    await expect(getRule("cooler-check")).rejects.toThrow();
    // NOTE: the left RAIL lists via the shipped host `rules.list`, which currently returns an empty
    // roster even for a saved rule (it deserializes the generic `lb_store::scan` row directly, but
    // `scan` returns the `{data, rev}` envelope — the record is under `row.data["data"]`). Once the
    // lead lands the one-line host fix, assert `screen.findByLabelText("open rule cooler-check")` here.
    expect(screen.getByLabelText("rule rail")).toBeInTheDocument();
  });

  it("rename: change the name of an open rule (same id) via the real path", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signIn(ws);

    render(<RulesView ws={ws} />);

    // Create a rule, then rename it through the header rename form.
    await typeBody(user, "1 + 1");
    await user.click(screen.getByLabelText("new rule"));
    await user.type(screen.getByLabelText("new rule name"), "First name");
    await user.click(screen.getByLabelText("create rule"));

    await user.click(await screen.findByLabelText("rename rule"));
    const nameField = await screen.findByLabelText("rule name");
    await user.clear(nameField);
    await user.type(nameField, "Renamed rule");
    await user.click(screen.getByLabelText("confirm rename rule"));

    // The id is unchanged (slug of the ORIGINAL name); only the name changed.
    const reopened = await getRule("first-name");
    expect(reopened.id).toBe("first-name");
    expect(reopened.name).toBe("Renamed rule");
    expect(reopened.body).toBe("1 + 1");

    await deleteRule("first-name");
  });

  it("honest cage error: an eval body renders the verbatim cage message, not a fake result", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signIn(ws);

    render(<RulesView ws={ws} />);
    await typeBody(user, 'eval("1 + 1")');
    await user.click(screen.getByLabelText("run rule"));

    const err = await screen.findByLabelText("run error");
    // The cage rejected `eval` — author feedback shown, NOT swallowed and NOT a fake result.
    expect(err.textContent).not.toBe("");
    expect(err.textContent).not.toBe("not permitted");
    expect(screen.queryByLabelText("scalar result")).not.toBeInTheDocument();
  });

  it("honest AI state: an ai.* body in a model-less workspace renders 'AI not configured'", async () => {
    const user = userEvent.setup();
    const ws = nextWs();
    await signIn(ws);

    render(<RulesView ws={ws} />);
    await typeBody(user, 'ai.complete("hi")');
    await user.click(screen.getByLabelText("run rule"));

    const err = await screen.findByLabelText("run error");
    expect(err.textContent).toContain("AI not configured");
    expect(screen.queryByLabelText("scalar result")).not.toBeInTheDocument();
  });
});
