import { describe, expect, it } from "vitest";
import {
  parseFacet,
  serializeFacet,
  remapFacetUids,
  exposedPorts,
  aliasLabel,
  parseAliasInput,
  rawFacet,
  FACET_PROP,
  type ComponentFacet,
} from "./facet";

// The control characters the facet wire format uses. Re-declared here (not
// exported from facet.ts) precisely so this test acts as a FORMAT LOCK: if the
// encoding ever changes, the byte-exact assertions below fail loudly. That's the
// regression guard for the "did the __facets format change?" class of bug.
const RS = "\x1e"; // between property records
const US = "\x1f"; // between fields within a record
const GS = "\x1d"; // between alias items
const FS = "\x1c"; // between an alias's code and its label

describe("facet round-trip", () => {
  it("parse ∘ serialize is identity for a fully-populated facet", () => {
    const f: ComponentFacet = new Map();
    f.set(101, {
      label: "Temp",
      unit: "°C",
      decimals: 1,
      min: 0,
      max: 100,
      order: 2,
      aliases: [
        { code: 0, label: "off" },
        { code: 1, label: "on" },
      ],
    });
    f.set(202, {
      // an exposed-port record (keyed by a child prop uid)
      label: "Port A",
      expose: "input",
      childComponent: 500,
      facetProp: 600,
    });
    expect(parseFacet(serializeFacet(f))).toEqual(f);
  });

  it("serialize is stable through a parse→serialize cycle", () => {
    const f: ComponentFacet = new Map();
    f.set(1, { label: "x", hidden: true, action: "pickMode" });
    f.set(2, { expose: "output", childComponent: 9, facetProp: 10 });
    const s = serializeFacet(f);
    expect(serializeFacet(parseFacet(s))).toBe(s);
  });

  it("round-trips a CHAINED exposed port (k field)", () => {
    const f: ComponentFacet = new Map();
    // chained: childComponent is the inner folder, chain flag set
    f.set(50, { label: "Port", expose: "output", childComponent: 99, facetProp: 88, chain: true });
    expect(parseFacet(serializeFacet(f))).toEqual(f);
  });

  it("round-trips the maintainer's owner (w) + name (m) fields", () => {
    const f: ComponentFacet = new Map();
    // a port with no user label — `name` is the canonical fallback (no #uid render)
    f.set(50, { expose: "input", childComponent: 9, facetProp: 88, chain: true, owner: 200, name: "in1" });
    const rt = parseFacet(serializeFacet(f));
    expect(rt.get(50)?.owner).toBe(200);
    expect(rt.get(50)?.name).toBe("in1");
    expect(rt).toEqual(f);
  });

  it("empty input parses to an empty map; empty records are dropped on serialize", () => {
    expect(parseFacet("").size).toBe(0);
    const f: ComponentFacet = new Map();
    f.set(7, {}); // no fields → must not survive serialize
    expect(serializeFacet(f)).toBe("");
  });
});

describe("facet wire format (byte-exact lock)", () => {
  it("encodes a label/unit/aliases record with the documented delimiters", () => {
    const f: ComponentFacet = new Map();
    f.set(42, {
      label: "Mode",
      unit: "x",
      aliases: [
        { code: 0, label: "auto" },
        { code: 1, label: "manual" },
      ],
    });
    const expected =
      `42${US}lMode${US}ux${US}o0${FS}auto${GS}1${FS}manual`;
    expect(serializeFacet(f)).toBe(expected);
  });

  it("separates multiple property records with RS", () => {
    const f: ComponentFacet = new Map();
    f.set(1, { label: "a" });
    f.set(2, { label: "b" });
    expect(serializeFacet(f)).toBe(`1${US}la${RS}2${US}lb`);
  });
});

describe("exposedPorts", () => {
  it("returns only records carrying an expose side, with the child prop uid", () => {
    const facet = parseFacet(
      serializeFacet(
        new Map([
          [10, { label: "plain" }],
          [20, { expose: "input", childComponent: 1, facetProp: 2 }],
          [30, { expose: "output", childComponent: 3, facetProp: 4 }],
        ]),
      ),
    );
    const ports = exposedPorts(facet).sort((a, b) => a.childUid - b.childUid);
    expect(ports.map((p) => [p.childUid, p.side])).toEqual([
      [20, "input"],
      [30, "output"],
    ]);
  });
});

describe("aliasLabel", () => {
  const aliases = [
    { code: 0, label: "off" },
    { code: 1, label: "on" },
  ];
  it("maps booleans to 0/1 codes", () => {
    expect(aliasLabel(aliases, false)).toBe("off");
    expect(aliasLabel(aliases, true)).toBe("on");
  });
  it("maps numbers directly and returns undefined on a miss", () => {
    expect(aliasLabel(aliases, 1)).toBe("on");
    expect(aliasLabel(aliases, 2)).toBeUndefined();
    expect(aliasLabel(undefined, 1)).toBeUndefined();
  });
});

describe("parseAliasInput", () => {
  it("parses a comma list of code=label pairs", () => {
    expect(parseAliasInput("0=off, 1=auto, 2=manual")).toEqual([
      { code: 0, label: "off" },
      { code: 1, label: "auto" },
      { code: 2, label: "manual" },
    ]);
  });
  it("skips blank and malformed parts and non-numeric codes", () => {
    expect(parseAliasInput("0=off, , 1, x=bad, 2=on")).toEqual([
      { code: 0, label: "off" },
      { code: 2, label: "on" },
    ]);
  });
  it("round-trips with the alias label resolver", () => {
    const aliases = parseAliasInput("0=off,1=on");
    expect(aliasLabel(aliases, true)).toBe("on");
  });
});

describe("rawFacet", () => {
  it("reads the __facets string off a component's properties", () => {
    expect(rawFacet({ [FACET_PROP]: { value: "abc" } })).toBe("abc");
    expect(rawFacet({ [FACET_PROP]: { value: 123 } })).toBeUndefined();
    expect(rawFacet({})).toBeUndefined();
    expect(rawFacet(undefined)).toBeUndefined();
  });
});

describe("remapFacetUids (deep-copy uid rewrite)", () => {
  it("rewrites the record key (prop), childComponent (comp) and facetProp (prop)", () => {
    const original = serializeFacet(
      new Map([
        [100, { label: "own", aliases: [{ code: 1, label: "on" }] }],
        [200, { expose: "input", childComponent: 500, facetProp: 600 }],
      ]),
    );
    const compMap = new Map([[500, 5500]]);
    const propMap = new Map([
      [100, 1100],
      [200, 2200],
      [600, 6600],
    ]);
    const remapped = parseFacet(remapFacetUids(original, compMap, propMap));

    // own record moved to its new prop uid, payload preserved
    expect(remapped.get(1100)?.label).toBe("own");
    expect(remapped.has(100)).toBe(false);

    // exposed-port record: key + childComponent + facetProp all rewritten
    const port = remapped.get(2200);
    expect(port?.childComponent).toBe(5500);
    expect(port?.facetProp).toBe(6600);
    expect(remapped.has(200)).toBe(false);
  });

  it("leaves uids absent from the maps untouched", () => {
    const original = serializeFacet(new Map([[100, { label: "x" }]]));
    expect(remapFacetUids(original, new Map(), new Map())).toBe(original);
  });

  it("round-trips the datetime format field", () => {
    const raw = serializeFacet(new Map([[100, { label: "Next", format: "datetime" }]]));
    expect(parseFacet(raw).get(100)?.format).toBe("datetime");
    // an unknown format value is ignored
    expect(parseFacet("100\x1ftbogus").get(100)?.format).toBeUndefined();
  });
});
