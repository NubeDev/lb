// The shell source-picker's Rules group, driven against a REAL in-process gateway (rules-as-source
// scope; CLAUDE §9 / testing §0 — no fake backend). Proves the mandatory paths for a rule-as-source:
//   - a saved rule surfaces in the picker as a `rules.run {rule_id}` READ source (the Data Studio
//     query-with-a-rule → chart path),
//   - WORKSPACE ISOLATION: a rule saved in `acme` is NOT offered to `beta` (the hard wall; §6),
//   - CAPABILITY-DENY: a workspace without `mcp:rules.list:call` gets an EMPTY Rules group, never a
//     crash and never a fabricated entry (deny-tolerant loader, §9).
// It drives the package's pure `loadSourcePicker` through the SHELL loaders (`shellLoaders` via the
// public `useSourcePicker` seam is React; here we call the same client fns the adapter injects) against
// the real `rules.*` host verbs + real store — the transport-agnostic package's ws-wall lands here,
// where a node is in-process.

import { describe, expect, it, beforeAll } from "vitest";
import { loadSourcePicker } from "@nube/source-picker";

import { listRules, saveRule, runRule, getRule } from "@/lib/rules";
import { useRealGateway, signInWithCaps } from "@/test/gateway-session";

let n = 0;
const nextWs = () => `rules-src-${n++}`;

// A records-returning rule: `frame`/`f.records()` shape is the chart-ready output a data panel draws.
const DATA_RULE_BODY = `let rows = [#{ h: 0, v: 1 }, #{ h: 1, v: 2 }]; rows`;

const LIST_CAPS = [
  "mcp:rules.list:call",
  "mcp:rules.save:call",
  "mcp:rules.run:call",
  "mcp:rules.get:call",
  "store:rule:read",
  "store:rule:write",
];
// No `mcp:rules.list:call` — the deny path.
const NO_LIST_CAPS = ["mcp:rules.save:call", "store:rule:read", "store:rule:write"];

beforeAll(() => {
  useRealGateway();
});

/** The rule-source subset of the shell's injected loaders — only `listRules` matters here. */
const rulesLoaders = { listRules: () => listRules() };

describe("source picker · Rules group (real gateway)", () => {
  it("offers a saved rule as a rules.run read source", async () => {
    const ws = nextWs();
    await signInWithCaps("user:ada", ws, LIST_CAPS);
    await saveRule({ id: "hourly", name: "Hourly mean", body: DATA_RULE_BODY });

    const { entries } = await loadSourcePicker(rulesLoaders);
    const rule = entries.find((e) => e.group === "rules");
    expect(rule?.label).toBe("Hourly mean");
    expect(rule?.source).toEqual({ tool: "rules.run", args: { rule_id: "hourly" } });
  });

  it("does NOT offer a rule from another workspace (the hard wall)", async () => {
    const acme = nextWs();
    await signInWithCaps("user:ada", acme, LIST_CAPS);
    await saveRule({ id: "secret", name: "Acme only", body: DATA_RULE_BODY });

    const beta = nextWs();
    await signInWithCaps("user:bob", beta, LIST_CAPS);
    const { entries } = await loadSourcePicker(rulesLoaders);
    expect(entries.some((e) => e.id === "rule:secret")).toBe(false);
    expect(entries.some((e) => e.group === "rules")).toBe(false); // beta has no rules of its own
  });

  it("carries a rule's declared params onto the picker entry", async () => {
    const ws = nextWs();
    await signInWithCaps("user:ada", ws, LIST_CAPS);
    await saveRule({
      id: "by-site",
      name: "By site",
      body: `[#{ site: param("site") }]`,
      params: [{ name: "site", label: "Site" }],
    });

    const { entries } = await loadSourcePicker(rulesLoaders);
    const rule = entries.find((e) => e.id === "rule:by-site");
    // The node serializes the full param shape (kind/required/options serde-default on an untyped save).
    expect(rule?.params).toEqual([
      { name: "site", label: "Site", kind: "text", required: false, options: [] },
    ]);
  });

  it("persists a TYPED param through save → get → picker entry (the authoring loop)", async () => {
    const ws = nextWs();
    await signInWithCaps("user:ada", ws, LIST_CAPS);
    // Exactly the shape the ParamDeclEditor builds: a required enum + a number.
    await saveRule({
      id: "typed",
      name: "Typed",
      body: `[#{ region: param("region"), hours: param("hours") }]`,
      params: [
        { name: "region", label: "Region", kind: "enum", required: true, options: ["emea", "amer"] },
        { name: "hours", kind: "number" },
      ],
    });

    // The record persisted the typed declaration (the node serializes all fields; match the ones we set).
    const saved = await getRule("typed");
    expect(saved.params[0]).toMatchObject({
      name: "region",
      label: "Region",
      kind: "enum",
      required: true,
      options: ["emea", "amer"],
    });
    expect(saved.params[1]).toMatchObject({ name: "hours", kind: "number" });

    // And the picker entry carries it (so the Data Studio form renders typed inputs).
    const { entries } = await loadSourcePicker(rulesLoaders);
    const entry = entries.find((e) => e.id === "rule:typed");
    expect(entry?.params?.[0]).toMatchObject({ name: "region", kind: "enum", required: true });
    expect(entry?.params?.[1]).toMatchObject({ name: "hours", kind: "number" });
  });

  it("runs a param-driven rule end to end (the param reaches the cage output)", async () => {
    const ws = nextWs();
    await signInWithCaps("user:ada", ws, LIST_CAPS);
    // The rule echoes its `site` param into a row — proves `args.params` flows through `rules.run`.
    await saveRule({
      id: "echo",
      name: "Echo site",
      body: `[#{ site: param("site") }]`,
      params: [{ name: "site" }],
    });

    // This is exactly the shape the Query tab's params form builds: `args.params` on the rules.run target.
    const result = await runRule({ ruleId: "echo", params: { site: "acme-hq" } });
    // The output is a grid; the single row carries the param value the form supplied.
    expect(JSON.stringify(result.output)).toContain("acme-hq");
  });

  it("preserves a NUMBER param's JSON type into the cage (adds, not concatenates)", async () => {
    const ws = nextWs();
    await signInWithCaps("user:ada", ws, LIST_CAPS);
    // `param("n") + 1` is 25 if `n` arrived as a number, "241" if it arrived as a string — the proof
    // the form's number coercion + host type-preservation reach the cage as a real number.
    await saveRule({ id: "add", name: "Add", body: `[#{ total: param("n") + 1 }]`, params: [{ name: "n", kind: "number" }] });
    const result = await runRule({ ruleId: "add", params: { n: 24 } });
    const s = JSON.stringify(result.output);
    expect(s).toContain("25");
    expect(s).not.toContain("241");
  });

  it("yields an EMPTY Rules group without the rules.list cap (deny-tolerant)", async () => {
    const ws = nextWs();
    // Save a rule WITH the save cap, then re-sign into the same ws WITHOUT the list cap.
    await signInWithCaps("user:ada", ws, LIST_CAPS);
    await saveRule({ id: "present", name: "Present", body: DATA_RULE_BODY });
    await signInWithCaps("user:ada", ws, NO_LIST_CAPS);

    const { entries } = await loadSourcePicker(rulesLoaders);
    expect(entries.some((e) => e.group === "rules")).toBe(false); // denied read → empty, no crash
  });
});
