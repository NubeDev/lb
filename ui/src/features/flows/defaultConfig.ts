// Per-node-type default config seeds (flows-canvas). When an author drags a node onto the canvas,
// `addNode` calls `defaultConfig(type)` to pre-fill the config buffer with a sensible starting point
// for that node kind — so a freshly-added `rhai` node already shows a working template instead of a
// blank source box. One responsibility: map a node type → its seed config object (no effects).

/** The rhai node's starter source — a tiny shape-router over the incoming payload:
 *  number → ×100, bool → "on"/"off", anything else → the type name as a string. */
const RHAI_TEMPLATE = `// Read the incoming payload from the wire (the envelope's primary value).
// Each envelope field is a top-level variable: payload, topic, ...
let value = payload;

// Number -> scale by 100 (covers both integer and float payloads).
if type_of(value) == "i64" || type_of(value) == "f64" {
    return value * 100;
}
// Bool -> "on" for true, "off" for false (a downstream node can route on the string).
else if type_of(value) == "bool" {
    return if value { "on" } else { "off" };
}
// Anything else (string, array, object, null) -> echo its type name as a string.
else {
    return type_of(value);
}
`;

/** The seed config for a freshly-added node of `type`. Returns `{}` when the type has no starter
 *  template (the common case — most nodes start empty and the SchemaForm renders their defaults). */
export function defaultConfig(type: string): Record<string, unknown> {
  switch (type) {
    case "rhai":
      return { source: RHAI_TEMPLATE };
    default:
      return {};
  }
}
