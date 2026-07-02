const a = 1;
function l(e) {
  return typeof e == "object" && e !== null && typeof e.$bind == "string";
}
function f(e) {
  let o = e.v ? e : { ...e, v: 1 };
  for (; o.v < 1; )
    switch (o.v) {
      default:
        return o;
    }
  return o;
}
function d(e, o) {
  const n = [], { catalog: c } = o;
  typeof e.v != "number" ? n.push({ level: "error", code: "missing-version", message: "IR spec has no numeric `v`" }) : e.v > 1 && n.push({
    level: "error",
    code: "future-version",
    message: `IR spec v${e.v} is newer than this renderer (v1)`
  });
  const s = new Set(Object.keys(e.components));
  !e.surface || typeof e.surface.root != "string" || e.surface.root === "" ? n.push({ level: "error", code: "no-root", message: "surface has no root component" }) : s.has(e.surface.root) || n.push({
    level: "error",
    code: "dangling-root",
    message: `surface root "${e.surface.root}" is not a defined component`
  });
  for (const [r, t] of Object.entries(e.components)) {
    t.id !== r && n.push({ level: "error", code: "id-mismatch", message: `component key "${r}" != id "${t.id}"`, componentId: r }), c.has(t.component) || n.push({
      level: "error",
      code: "unknown-component",
      message: `component "${t.component}" (id ${r}) is not in the catalog`,
      componentId: r
    });
    for (const i of t.children ?? [])
      s.has(i) || n.push({
        level: "warning",
        code: "dangling-child",
        message: `child "${i}" of "${r}" is not a defined component`,
        componentId: r
      });
  }
  return n;
}
function u(e) {
  return e.filter((o) => o.level === "error");
}
function m(e) {
  return e.filter((o) => o.level === "warning");
}
export {
  a as I,
  u as e,
  l as i,
  f as m,
  d as v,
  m as w
};
