// Folder exposed-port boundary + chain — the shared algorithm (see
// ../../EXPOSURE_SPEC.md). This is the JS half of a two-implementation contract
// (the C++ engine Folder component is the other); both are verified against the
// SAME golden vectors (exposure-vectors.json). Keep this pure and in lockstep
// with the spec — don't change behaviour without a vector.
//
// `computeFolderExposure` decides which child props a folder exposes as ports
// (and which are CHAINED through an inner folder). The engine owns this at
// runtime; the client keeps it for the dev harness, optimistic preview, and as
// the executable spec. `resolveChain` walks a chained port down to its real owner.

export interface ExposureRecord {
  prop: number; // the DEEP prop uid — the record key (always)
  side: "input" | "output";
  childComponent: number; // real owner (direct) OR inner folder (chain link)
  facetProp?: number; // childComponent's __facets prop uid
  chain: boolean; // true when childComponent is an inner folder
}

export interface ExposureInput {
  folder: number;
  // uid → parentUid. JSON object keys arrive as strings, so look up both.
  parents: Record<string | number, number>;
  edges: Array<{ s: number; sp: number; t: number; tp: number }>;
  facetProp?: Record<string | number, number>;
}

// Bound every hierarchy walk so a broken/cyclic parent map can't hang.
const MAX_DEPTH = 256;

function lookup(m: Record<string | number, number> | undefined, uid: number): number | undefined {
  if (!m) return undefined;
  return m[uid] ?? m[String(uid)];
}

// `x` is a STRICT descendant of `folder` (the folder itself is not "inside").
function isInside(x: number, folder: number, parents: ExposureInput["parents"]): boolean {
  let cur = x;
  for (let i = 0; i < MAX_DEPTH; i++) {
    const p = lookup(parents, cur);
    if (p === undefined) return false;
    if (p === folder) return true;
    cur = p;
  }
  return false;
}

// The DIRECT child of `folder` on the path down to `cin` (walk up until the
// parent is `folder`). Returns `cin` itself when it's already a direct child.
function viaChild(cin: number, folder: number, parents: ExposureInput["parents"]): number | undefined {
  let cur = cin;
  for (let i = 0; i < MAX_DEPTH; i++) {
    const p = lookup(parents, cur);
    if (p === undefined) return undefined;
    if (p === folder) return cur;
    cur = p;
  }
  return undefined;
}

// The exposed-port records a folder should carry: one per child prop with an
// edge crossing the folder's boundary. Deep owners are CHAINED through the
// direct-child folder they sit in. Sorted by prop for stable output.
export function computeFolderExposure(input: ExposureInput): ExposureRecord[] {
  const { folder, parents, edges } = input;
  const byProp = new Map<number, ExposureRecord>();
  for (const e of edges) {
    const sIn = isInside(e.s, folder, parents);
    const tIn = isInside(e.t, folder, parents);
    if (sIn === tIn) continue; // internal or fully external — not this folder's boundary
    const cin = sIn ? e.s : e.t;
    const pin = sIn ? e.sp : e.tp;
    const side: "input" | "output" = sIn ? "output" : "input";
    const via = viaChild(cin, folder, parents);
    if (via === undefined) continue; // defensive: broken parent map
    const chain = via !== cin;
    const link = chain ? via : cin; // chain → the inner folder; direct → the owner
    const f = lookup(input.facetProp, link);
    const rec: ExposureRecord = { prop: pin, side, childComponent: link, chain };
    if (f !== undefined) rec.facetProp = f; // omit (don't null) when unknown
    byProp.set(pin, rec);
  }
  return [...byProp.values()].sort((a, b) => a.prop - b.prop);
}

// Walk a (possibly chained) port down to its REAL owner component — e.g. when
// drawing a new edge to it. `recordFor(component, prop)` returns that
// component's exposure record for `prop`, or undefined. Bounded (cycle guard).
export function resolveChain(
  recordFor: (component: number, prop: number) => ExposureRecord | undefined,
  folder: number,
  prop: number,
): number | undefined {
  let comp = folder;
  for (let i = 0; i < MAX_DEPTH; i++) {
    const r = recordFor(comp, prop);
    if (!r) return undefined;
    if (!r.chain) return r.childComponent; // direct record → real owner
    comp = r.childComponent; // follow one link inward
  }
  return undefined;
}
