// Per-component `__facet` presentation metadata: parse / serialize / cache.
//
// `__facet` is just an input string property on each component (systemRole
// ROLE_FACETS) whose value describes how to present the component's OTHER
// properties — labels, units, number formatting, and value→label aliases.
// See FACET_DESIGN.md for the full design. Control-char delimited so it's a
// cheap split-parse (no JSON); both the engine and the UI write it.

// Delimiters (never appear in user text → no escaping).
const RS = "\x1e"; // between property records
const US = "\x1f"; // between fields within a record
const GS = "\x1d"; // between alias / option items
const FS = "\x1c"; // between an alias's code and its label

// The property name that carries the facet string on every component.
export const FACET_PROP = "__facets";

// Display decimals are clamped to this — more is meaningless for a UI (and beyond
// 100 toFixed throws). Keeps a fat-fingered "123" from rendering a 100-digit value.
export const MAX_DECIMALS = 10;

export interface Alias {
  code: number; // the property's native value (int; bool → 0/1)
  label: string;
}

export interface PropFacet {
  label?: string;
  unit?: string;
  decimals?: number;
  min?: number;
  max?: number;
  hidden?: boolean;
  order?: number;
  // `t` — render a numeric (epoch) value as a LOCAL date/time instead of a raw
  // number. Engine stores epochs in UTC; this is the display-side conversion.
  format?: "datetime" | "date" | "time";
  aliases?: Alias[]; // `o` — value→label map (also the pick list)
  action?: string; // `a` — dynamic-options action (Phase 3; not used yet)
  // `e` — exposed port: this record's uid is a CHILD prop projected onto THIS
  // component as an input/output port (see FACET_DESIGN.md §9). Value streams via
  // a prop-level subscription (key uid alone); but edges/overrides need the child
  // COMPONENT uid too (no fast prop→component lookup engine-side) — that's `c`.
  expose?: "input" | "output";
  childComponent?: number; // `c` — owning component of an exposed prop
  facetProp?: number; // `f` — the child's __facets prop uid (sub it for LIVE facet)
  // `k` — chained exposure (grouping-of-groups): this port re-projects an INNER
  // folder's already-exposed port, so the prop stays exposed exactly once by its
  // direct parent. When set, `childComponent` is the INNER FOLDER (the next chain
  // link) and `facetProp` its __facets; follow that folder's record for the same
  // prop uid to reach the real owner. See FACET_DESIGN.md §9.
  chain?: boolean;
  // `w` — the REAL deep owner component uid (for a chained port `childComponent` is
  // the inner-folder link, `owner` the terminal owner). `m` — the owner prop's name,
  // the canonical fallback label so an engine-written port renders with its name
  // instead of `#<uid>`. Both written by the maintainer. (EXPOSURE_SPEC §7.)
  owner?: number;
  name?: string;
}

export type ComponentFacet = Map<number /* propUid */, PropFacet>;

export function parseFacet(raw: string): ComponentFacet {
  const out: ComponentFacet = new Map();
  if (!raw) return out;
  for (const rec of raw.split(RS)) {
    if (!rec) continue;
    const fields = rec.split(US);
    const uid = Number(fields[0]);
    if (!Number.isFinite(uid)) continue;
    const f: PropFacet = {};
    for (let i = 1; i < fields.length; i++) {
      const fld = fields[i];
      if (!fld) continue;
      const v = fld.slice(1);
      switch (fld[0]) {
        case "l": f.label = v; break;
        case "u": f.unit = v; break;
        case "d": {
          // Clamp to a sane range so a bad stored value can't crash the
          // formatters (toFixed > 100 throws) or render absurd precision.
          const d = Number(v);
          if (Number.isFinite(d)) f.decimals = Math.min(MAX_DECIMALS, Math.max(0, Math.trunc(d)));
          break;
        }
        case "n": f.min = Number(v); break;
        case "x": f.max = Number(v); break;
        case "h": f.hidden = v !== "0"; break;
        case "r": f.order = Number(v); break;
        case "t": if (v === "datetime" || v === "date" || v === "time") f.format = v; break;
        case "a": f.action = v; break;
        case "e": f.expose = v === "o" ? "output" : "input"; break;
        case "c": f.childComponent = Number(v); break;
        case "f": f.facetProp = Number(v); break;
        case "k": f.chain = v !== "0"; break;
        case "w": f.owner = Number(v); break;
        case "m": f.name = v; break;
        case "o":
          f.aliases = v.split(GS).map((o) => {
            const j = o.indexOf(FS);
            return j < 0
              ? { code: Number(o), label: o }
              : { code: Number(o.slice(0, j)), label: o.slice(j + 1) };
          });
          break;
      }
    }
    out.set(uid, f);
  }
  return out;
}

export function serializeFacet(facet: ComponentFacet): string {
  const recs: string[] = [];
  for (const [uid, f] of facet) {
    const fields: string[] = [String(uid)];
    if (f.label) fields.push("l" + f.label);
    if (f.unit) fields.push("u" + f.unit);
    if (f.decimals != null) fields.push("d" + f.decimals);
    if (f.min != null) fields.push("n" + f.min);
    if (f.max != null) fields.push("x" + f.max);
    if (f.hidden) fields.push("h1");
    if (f.order != null) fields.push("r" + f.order);
    if (f.format) fields.push("t" + f.format);
    if (f.action) fields.push("a" + f.action);
    if (f.expose) fields.push("e" + (f.expose === "output" ? "o" : "i"));
    if (f.childComponent != null) fields.push("c" + f.childComponent);
    if (f.facetProp != null) fields.push("f" + f.facetProp);
    if (f.chain) fields.push("k1");
    if (f.owner != null) fields.push("w" + f.owner);
    if (f.name) fields.push("m" + f.name);
    if (f.aliases && f.aliases.length) {
      fields.push("o" + f.aliases.map((a) => a.code + FS + a.label).join(GS));
    }
    if (fields.length > 1) recs.push(fields.join(US)); // skip empty records
  }
  return recs.join(RS);
}

// Rewrite the uid references in a facet after a deep copy (which mints new uids
// but copies the facet value verbatim). Remaps the record KEY (a prop uid — own
// or, for an exposed port, the child's) plus the exposed-port `childComponent`
// (`c`) and `facetProp` (`f`). Uses old→new maps from the copy's uidMap. Refs not
// present in a map are left as-is. Returns the rewritten facet string.
export type UidMap = Map<number, number> | Record<number | string, number>;
const mapUid = (m: UidMap, k: number): number =>
  (m instanceof Map ? m.get(k) : m[k]) ?? k;

export function remapFacetUids(raw: string, compMap: UidMap, propMap: UidMap): string {
  const facet = parseFacet(raw);
  if (facet.size === 0) return raw;
  const out: ComponentFacet = new Map();
  for (const [propUid, f] of facet) {
    const nf: PropFacet = { ...f };
    if (nf.childComponent != null) nf.childComponent = mapUid(compMap, nf.childComponent);
    if (nf.facetProp != null) nf.facetProp = mapUid(propMap, nf.facetProp);
    out.set(mapUid(propMap, propUid), nf);
  }
  return serializeFacet(out);
}

// Per-component parse cache — re-parse only when a component's raw facet string
// actually changes. Bounded by the number of components.
const cache = new Map<number, { raw: string; parsed: ComponentFacet }>();

export function facetFor(componentUid: number, raw: string | undefined): ComponentFacet {
  const key = raw ?? "";
  const hit = cache.get(componentUid);
  if (hit && hit.raw === key) return hit.parsed;
  const parsed = parseFacet(key);
  cache.set(componentUid, { raw: key, parsed });
  return parsed;
}

// Read the raw facet string off a component's REST properties (or undefined).
export function rawFacet(
  properties: Record<string, { value: unknown }> | undefined,
): string | undefined {
  const v = properties?.[FACET_PROP]?.value;
  return typeof v === "string" ? v : undefined;
}

export interface ExposedPort {
  childUid: number; // a CHILD component's prop uid, projected onto the parent
  side: "input" | "output";
  facet: PropFacet; // the record's own metadata (label/unit/aliases/order)
}

// The child-prop ports this component exposes (facet records with an `e` field).
export function exposedPorts(facet: ComponentFacet): ExposedPort[] {
  const out: ExposedPort[] = [];
  for (const [uid, f] of facet) {
    if (f.expose) out.push({ childUid: uid, side: f.expose, facet: f });
  }
  return out;
}

// Parse the Configure panel's alias text field ("0=off, 1=auto, 2=manual") into
// Alias records. Skips blank / malformed parts and non-numeric codes.
export function parseAliasInput(s: string): Alias[] {
  const out: Alias[] = [];
  for (const part of s.split(",")) {
    const t = part.trim();
    if (!t) continue;
    const j = t.indexOf("=");
    if (j < 0) continue;
    const code = Number(t.slice(0, j).trim());
    const label = t.slice(j + 1).trim();
    if (Number.isFinite(code) && label) out.push({ code, label });
  }
  return out;
}

// Resolve a property's native value to its alias label, if the facet aliases it.
export function aliasLabel(aliases: Alias[] | undefined, value: unknown): string | undefined {
  if (!aliases || aliases.length === 0) return undefined;
  const code =
    value === true ? 1 : value === false ? 0 : typeof value === "number" ? value : Number(value);
  return aliases.find((a) => a.code === code)?.label;
}
