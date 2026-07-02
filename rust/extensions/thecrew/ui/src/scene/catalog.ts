// The machine- + agent-readable shape CATALOG (graphics-canvas phase 3: teaching-error validation).
// A validation error is only useful to an AI if it can name the failing shape AND say what WAS
// available — "unknown type hvac.fann" plus the real catalog lets the agent self-correct in one step
// (the Awaken normalize-then-teach lesson, parent scope §Risks "errors must teach"). This module turns
// the live symbol registry (the same `SymbolDef`s the renderer + PropertyRail read — one source of
// truth, never a hand-maintained second list) into a compact catalog the validator appends to its
// issue report and the SKILL doc mirrors.
//
// No three.js import: it reads only the `SymbolDef` metadata (type/label/propSchema/bindSlots), not the
// mesh components — so it unit-tests without a GL context. It imports the shape `*Def`s DIRECTLY (which
// depend only on `shape-props`/`scene.types`), NOT `ShapeNode.SYMBOLS`: ShapeNode → scene-store →
// validate → catalog would be a cycle (validate leans on this catalog to teach). The def list is the
// same one ShapeNode assembles `SYMBOLS` from — one source of truth, no cycle.

import type { SymbolDef } from "../canvas/shapes/shape-props";
import { ductDef } from "../canvas/shapes/Duct";
import { fanDef } from "../canvas/shapes/Fan";
import { damperDef } from "../canvas/shapes/Damper";
import { filterDef } from "../canvas/shapes/Filter";
import { coilDef } from "../canvas/shapes/Coil";
import { casingDef } from "../canvas/shapes/AhuCasing";
import { wallDef } from "../canvas/shapes/Wall";
import { roomDef } from "../canvas/shapes/Room";
import { doorDef } from "../canvas/shapes/Door";
import { labelDef } from "../canvas/shapes/Label";

/** The registry, assembled from the SAME def list ShapeNode uses for `SYMBOLS` (kept in sync by the
 *  shared `catalog-defs.test.ts` assertion). Local to break the ShapeNode→store→validate→catalog cycle. */
const DEFS: SymbolDef[] = [
  ductDef, fanDef, damperDef, filterDef, coilDef, casingDef, wallDef, roomDef, doorDef, labelDef,
];

/** One catalog entry — everything an author (human or agent) needs to place + bind a shape. */
export interface CatalogEntry {
  type: string;
  label: string;
  /** prop name → a terse spec (kind + options), the shape's authorable `props`. */
  props: Record<string, { kind: string; options?: string[] }>;
  /** the prop names that accept a `bind` (each `bind[slot] = { channel: "<series>" }`). */
  bindSlots: string[];
}

/** Describe one symbol def as a catalog entry (prop schema flattened to kind/options). */
function entryOf(def: SymbolDef): CatalogEntry {
  const props: CatalogEntry["props"] = {};
  for (const [name, spec] of Object.entries(def.propSchema)) {
    props[name] = spec.options ? { kind: spec.kind, options: spec.options } : { kind: spec.kind };
  }
  return { type: def.type, label: def.label, props, bindSlots: def.bindSlots };
}

/** The full catalog, one entry per registered symbol type, in registry order. */
export function describeCatalog(): CatalogEntry[] {
  return DEFS.map(entryOf);
}

/** The bare list of known type names — the cheap thing an "unknown type" error cites. */
export function knownTypes(): string[] {
  return DEFS.map((d) => d.type);
}

/** A one-line-per-type human/agent rendering of the catalog for a teaching error or the skill doc:
 *  `hvac.fan — props: label(text), diameter(number), direction(select: left|right); bind: running, speed, fault`.
 *  Compact on purpose: it rides inside an error string the agent reads and acts on. */
export function catalogText(): string {
  return describeCatalog()
    .map((e) => {
      const props = Object.entries(e.props)
        .map(([n, s]) => (s.options ? `${n}(${s.kind}: ${s.options.join("|")})` : `${n}(${s.kind})`))
        .join(", ");
      const bind = e.bindSlots.length ? `; bind: ${e.bindSlots.join(", ")}` : "";
      return `${e.type} — props: ${props}${bind}`;
    })
    .join("\n");
}
