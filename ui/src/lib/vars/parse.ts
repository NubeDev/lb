// Extract the variable names a template references (widget-config-vars scope, "parse.ts"). Used to
// compute a cell's variable dependencies (refresh: which cells re-run when a variable changes) and the
// deny-set (which tools/subjects a variable feeds). Pure TS, no React.
//
// Recognises the three Grafana reference forms — `$name`, `${name}` (optionally `${name:format}`), and
// `[[name]]` (optionally `[[name:format]]`). Built-ins (`$__from`, `${__user.login}`, …) are returned
// too (a leading `__` name is a built-in); the caller filters them out of user-variable deps as needed.

/** A `$name` / `${name}` / `[[name]]` reference. A name is `[A-Za-z_][\w.]*` (dots allow `__user.login`). */
const NAME = "[A-Za-z_][\\w.]*";
const RE = new RegExp(
  // ${name} or ${name:format}
  `\\$\\{(${NAME})(?::[a-z]+)?\\}` +
    // [[name]] or [[name:format]]
    `|\\[\\[(${NAME})(?::[a-z]+)?\\]\\]` +
    // $name (not followed by `{` — that's the brace form)
    `|\\$(${NAME})`,
  "g",
);

/** Every distinct variable name referenced in `template` (in first-seen order), built-ins included. */
export function extractVarNames(template: string): string[] {
  const out: string[] = [];
  const seen = new Set<string>();
  let m: RegExpExecArray | null;
  RE.lastIndex = 0;
  while ((m = RE.exec(template)) !== null) {
    const name = m[1] ?? m[2] ?? m[3];
    if (name && !seen.has(name)) {
      seen.add(name);
      out.push(name);
    }
  }
  return out;
}

/** True if a name is a built-in global (`__from`, `__user.login`, …) rather than a user variable. */
export function isBuiltinName(name: string): boolean {
  return name.startsWith("__");
}

/** Walk a JSON value tree and collect every variable name referenced in any string leaf. The deep
 *  counterpart of `extractVarNames` over a cell's `source.args` / a JSON payload template. */
export function extractVarNamesDeep(node: unknown): string[] {
  const out: string[] = [];
  const seen = new Set<string>();
  const visit = (n: unknown) => {
    if (typeof n === "string") {
      for (const name of extractVarNames(n)) {
        if (!seen.has(name)) {
          seen.add(name);
          out.push(name);
        }
      }
    } else if (Array.isArray(n)) {
      n.forEach(visit);
    } else if (n && typeof n === "object") {
      Object.values(n).forEach(visit);
    }
  };
  visit(node);
  return out;
}
