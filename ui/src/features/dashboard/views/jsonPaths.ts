// JSON-path utilities for the flow value explorer (flow-dashboard-binding-ux-scope: "parse out the
// JSON"). A path is an array of segments — string keys (object) or numeric indices (array) — relative
// to a node's recorded value. These power the visual path picker (introspect → click a field → bind it)
// and the read-back extraction, agnostic to the shape (object / array / deeply nested / scalar leaf).

export type PathSeg = string | number;
export type JsonKind = "object" | "array" | "string" | "number" | "boolean" | "null";

/** The kind of a JSON value (drives the tree icons + which views fit). */
export function kindOf(v: unknown): JsonKind {
  if (v === null || v === undefined) return "null";
  if (Array.isArray(v)) return "array";
  const t = typeof v;
  if (t === "object") return "object";
  if (t === "number") return "number";
  if (t === "boolean") return "boolean";
  return "string";
}

/** Walk `path` into `root`; returns `undefined` if any segment is missing (an honest "not there"). */
export function valueAtPath(root: unknown, path: PathSeg[]): unknown {
  let cur: unknown = root;
  for (const seg of path) {
    if (cur == null) return undefined;
    if (typeof seg === "number") {
      if (!Array.isArray(cur)) return undefined;
      cur = cur[seg];
    } else {
      if (typeof cur !== "object" || Array.isArray(cur)) return undefined;
      cur = (cur as Record<string, unknown>)[seg];
    }
  }
  return cur;
}

/** One child of a container value, for the tree picker. */
export interface JsonChild {
  /** The segment that reaches this child from its parent. */
  seg: PathSeg;
  /** The display label (a key name, or `[i]` for an array index). */
  label: string;
  kind: JsonKind;
  value: unknown;
}

/** The directly-addressable children of a container (object keys / array indices). A leaf has none. */
export function childrenOf(v: unknown): JsonChild[] {
  if (Array.isArray(v)) {
    return v.map((item, i) => ({ seg: i, label: `[${i}]`, kind: kindOf(item), value: item }));
  }
  if (v && typeof v === "object") {
    return Object.entries(v as Record<string, unknown>).map(([k, val]) => ({
      seg: k,
      label: k,
      kind: kindOf(val),
      value: val,
    }));
  }
  return [];
}

/** A human, copy-pasteable path string: `payload.items[0].name` (root = `value`). */
export function pathLabel(path: PathSeg[]): string {
  if (path.length === 0) return "(whole value)";
  let out = "";
  for (const seg of path) {
    if (typeof seg === "number") out += `[${seg}]`;
    else out += out ? `.${seg}` : seg;
  }
  return out;
}

/** A short one-line preview of a value for a tree row (`42`, `"eco"`, `{3 keys}`, `[4]`). */
export function previewOf(v: unknown): string {
  switch (kindOf(v)) {
    case "object":
      return `{${Object.keys(v as object).length}}`;
    case "array":
      return `[${(v as unknown[]).length}]`;
    case "string":
      return JSON.stringify(v);
    case "null":
      return "null";
    default:
      return String(v);
  }
}

/** Parse a stored path (the source arg) — accepts an already-array path, or `null`/absent → `[]`. */
export function asPath(raw: unknown): PathSeg[] {
  if (Array.isArray(raw)) return raw.filter((s) => typeof s === "string" || typeof s === "number");
  return [];
}
