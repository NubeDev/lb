// Humanize a field NAME into a display label (widget-kit scope, Phase 1) — the FALLBACK a surface uses
// when a field author declared no `label`/`displayName` override. `maxRuns` → "Max Runs", `nextAttemptTs`
// → "Next Attempt Ts", `principal_sub` → "Principal Sub". This is the ONLY label source that isn't
// author-declared; a label override always wins (see `resolve.ts`). One responsibility: name → title case.

/** Turn a camelCase / snake_case / kebab-case field name into Title Case words.
 *  `maxRuns` → "Max Runs"; `nextAttemptTs` → "Next Attempt Ts"; `action_kind` → "Action Kind". */
export function humanize(fieldName: string): string {
  const words = fieldName
    // camelCase / PascalCase boundary: insert a space before an interior capital (also splits `Ts`).
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    // acronym → word boundary (`HTTPServer` → `HTTP Server`).
    .replace(/([A-Z]+)([A-Z][a-z])/g, "$1 $2")
    // snake_case / kebab-case separators.
    .replace(/[_-]+/g, " ")
    .trim()
    .split(/\s+/)
    .filter((w) => w.length > 0);
  return words
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
    .join(" ");
}
