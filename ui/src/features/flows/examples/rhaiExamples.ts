// Ready-to-use example bodies for the `rhai` flow node, grouped by category (flows-canvas / rhai-node
// examples). A flow rhai node runs in the same cage the rules workbench uses, but its I/O is the
// message envelope: each envelope field (`payload`, `topic`, …) is a top-level variable, and the node
// returns a value OR emits an envelope downstream. These bodies teach that convention one concept at a
// time. This is a STATIC catalog (data, named by concept — not a `utils` dump); the panel below the
// editor renders them. Where a body calls a cage verb (`emit`/`alert`/`log`), it mirrors the same verb
// the rules examples prove green, so an example that runs here runs there too.

/** One example — a title, a one-line teaching note, and the rhai body. */
export interface RhaiExample {
  id: string;
  title: string;
  summary: string;
  body: string;
}

/** A named group of examples (rendered as a collapsible section). */
export interface RhaiExampleCategory {
  id: string;
  title: string;
  examples: RhaiExample[];
}

const j = (...lines: string[]) => lines.join("\n");

export const RHAI_EXAMPLE_CATEGORIES: RhaiExampleCategory[] = [
  {
    id: "basics",
    title: "Basics",
    examples: [
      {
        id: "passthrough",
        title: "Pass the payload through",
        summary: "The identity node — return the incoming payload unchanged.",
        body: "// Each envelope field is a top-level variable: payload, topic, ...\npayload",
      },
      {
        id: "scalar",
        title: "A constant value",
        summary: "Ignore the input and return a fixed value — the simplest possible node.",
        body: "42",
      },
      {
        id: "arithmetic",
        title: "Scale a number",
        summary: "Multiply a numeric payload by 100 (e.g. a 0–1 ratio → a percentage).",
        body: "payload * 100",
      },
      {
        id: "topic-read",
        title: "Read the topic",
        summary: "The envelope carries more than the payload — return the message topic.",
        body: "// `topic` is the envelope's routing key.\ntopic",
      },
    ],
  },
  {
    id: "shaping",
    title: "Shaping payloads",
    examples: [
      {
        id: "build-object",
        title: "Build a new object",
        summary: "Assemble a fresh map from the payload — reshape data for the next node.",
        body: j(
          "#{",
          "  value: payload,",
          "  topic: topic,",
          "  seen: true,",
          "}",
        ),
      },
      {
        id: "pick-field",
        title: "Pick a field from an object payload",
        summary: "Reach into a structured payload and return one field (with a fallback).",
        body: j(
          "// Object payloads are maps — index by key.",
          'if type_of(payload) == "map" {',
          '  payload["temperature"] ?? 0.0',
          "} else {",
          "  0.0",
          "}",
        ),
      },
      {
        id: "rename-fields",
        title: "Rename / remap fields",
        summary: "Map an inbound shape to the shape a downstream node expects.",
        body: j(
          "#{",
          '  id: payload["device_id"],',
          '  celsius: payload["temp_c"],',
          '  at: payload["ts"],',
          "}",
        ),
      },
      {
        id: "default-missing",
        title: "Default a missing field",
        summary: "Fill an absent field with a default using the `??` (null-coalescing) operator.",
        body: j(
          "let name = payload[\"name\"] ?? \"unknown\";",
          "#{ name: name, source: topic }",
        ),
      },
      {
        id: "stringify",
        title: "Format a string",
        summary: "Interpolate payload fields into a human message string.",
        body: '`device ${payload["id"]} reads ${payload["value"]}`',
      },
    ],
  },
  {
    id: "logic",
    title: "Conditions & routing",
    examples: [
      {
        id: "type-router",
        title: "Route by payload type",
        summary: "Branch on the payload's runtime type — the starter template's shape-router.",
        body: j(
          "let v = payload;",
          'if type_of(v) == "i64" || type_of(v) == "f64" { v * 100 }',
          'else if type_of(v) == "bool" { if v { "on" } else { "off" } }',
          "else { type_of(v) }",
        ),
      },
      {
        id: "threshold",
        title: "Threshold flag",
        summary: "Return a boolean a downstream node can gate on (true when over the limit).",
        body: 'payload["value"] > 5.0',
      },
      {
        id: "clamp",
        title: "Clamp to a range",
        summary: "Bound a numeric payload between a floor and a ceiling.",
        body: j(
          "let v = payload;",
          "if v < 0 { 0 } else if v > 100 { 100 } else { v }",
        ),
      },
      {
        id: "topic-switch",
        title: "Switch on the topic",
        summary: "Pick a label from the message topic — content-based routing.",
        body: j(
          'switch topic {',
          '  "sensors/temp" => "temperature",',
          '  "sensors/hum" => "humidity",',
          '  _ => "other",',
          "}",
        ),
      },
    ],
  },
  {
    id: "collections",
    title: "Arrays & aggregation",
    examples: [
      {
        id: "map-array",
        title: "Map over an array payload",
        summary: "Transform every element of a list payload (here: double each number).",
        body: "payload.map(|x| x * 2)",
      },
      {
        id: "filter-array",
        title: "Filter an array payload",
        summary: "Keep only the elements that pass a test (here: the positive numbers).",
        body: "payload.filter(|x| x > 0)",
      },
      {
        id: "sum-array",
        title: "Sum an array payload",
        summary: "Reduce a list of numbers to their total.",
        body: "payload.reduce(|sum, x| sum + x, 0)",
      },
      {
        id: "count",
        title: "Count elements",
        summary: "Return how many items the payload array holds.",
        body: "payload.len()",
      },
    ],
  },
  {
    id: "effects",
    title: "Findings & side-effects",
    examples: [
      {
        id: "log",
        title: "Log a line",
        summary: "Write a debug line (visible in the Debug tab) and pass the payload through.",
        body: 'log(`processing ${topic}`);\npayload',
      },
      {
        id: "emit-finding",
        title: "Emit a finding",
        summary: "Record a finding on the node's `findings` out-port — a structured observation.",
        body: 'emit(#{ level: "warning", msg: "value looks off", value: payload });',
      },
      {
        id: "alert-threshold",
        title: "Alert on a breach",
        summary: "Raise a critical alert only when the payload crosses a threshold.",
        body: j(
          'if payload["value"] > 5.0 {',
          '  alert(#{ level: "critical", msg: "ran hot", value: payload["value"] });',
          "}",
        ),
      },
      {
        id: "guarded-emit",
        title: "Emit then pass through",
        summary: "Flag a condition AND keep the data flowing — a finding plus a payload return.",
        body: j(
          'if payload["battery"] < 20 {',
          '  emit(#{ level: "warning", msg: "low battery" });',
          "}",
          "payload",
        ),
      },
    ],
  },
];

/** Every example flattened (for lookups / tests). */
export const RHAI_EXAMPLES: RhaiExample[] = RHAI_EXAMPLE_CATEGORIES.flatMap((c) => c.examples);
