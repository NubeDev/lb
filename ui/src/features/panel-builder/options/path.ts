// Dotted-path get/set for option values (editor-parity scope, step 2). An option's `path` (e.g.
// `custom.lineWidth`) addresses a nested value under its scope root; these read/write it immutably and
// PRUNE back to absent when a value is cleared, so an unset option never materializes an empty
// `{custom:{}}` and the round-trip stays byte-clean (the cellEditorState contract). Pure — no React.

/** Read `obj.a.b.c` for `path = "a.b.c"`, or `undefined` if any segment is missing. */
export function getPath(obj: Record<string, unknown> | undefined, path: string): unknown {
  let cur: unknown = obj;
  for (const seg of path.split(".")) {
    if (cur == null || typeof cur !== "object") return undefined;
    cur = (cur as Record<string, unknown>)[seg];
  }
  return cur;
}

/** Immutably set `obj.a.b.c = value` for `path = "a.b.c"`. A `value` of `undefined` DELETES the leaf
 *  and prunes any now-empty ancestor objects (so clearing an option leaves no empty `{custom:{}}`).
 *  Returns a new root; the input is untouched. */
export function setPath(
  obj: Record<string, unknown> | undefined,
  path: string,
  value: unknown,
): Record<string, unknown> {
  const segs = path.split(".");
  const root: Record<string, unknown> = { ...(obj ?? {}) };
  // Walk/clone down to the parent of the leaf.
  const parents: Array<{ node: Record<string, unknown>; key: string }> = [];
  let node = root;
  for (let i = 0; i < segs.length - 1; i++) {
    const key = segs[i];
    const child = node[key];
    const cloned: Record<string, unknown> =
      child != null && typeof child === "object" && !Array.isArray(child)
        ? { ...(child as Record<string, unknown>) }
        : {};
    node[key] = cloned;
    parents.push({ node, key });
    node = cloned;
  }
  const leaf = segs[segs.length - 1];
  if (value === undefined) delete node[leaf];
  else node[leaf] = value;

  // Prune now-empty ancestor objects (deepest first) so clearing prunes back to absent.
  for (let i = parents.length - 1; i >= 0; i--) {
    const { node: p, key } = parents[i];
    const child = p[key] as Record<string, unknown>;
    if (child && typeof child === "object" && Object.keys(child).length === 0) delete p[key];
  }
  return root;
}
