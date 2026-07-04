// Deep-parse embedded JSON strings in a run result (rules-editor-ux scope). A rule's result often
// carries a field whose VALUE is itself a JSON string — e.g. a `channel.history` row's `body` is
// `"{\"kind\":\"agent_result\",\"answer\":\"…\"}"` (the kind-tagged channel payload rides inside `body`
// as text). Rendered verbatim that reads as an escaped blob; parsed, it becomes a real nested object the
// tree view can expand. This walk turns any string that is *itself* valid JSON (an object or array — not
// a bare number/`true`/quoted word, which would strip meaning) into that value, recursively, so the
// viewer shows humane nesting while keeping every value faithful. One responsibility per file (FILE-LAYOUT).

/** Return `value` with every embedded JSON-object/array string parsed into a real nested value. */
export function parseEmbeddedJson(value: unknown): unknown {
  if (typeof value === "string") {
    const nested = tryParseJsonContainer(value);
    // A parsed container may itself carry more embedded JSON (a payload inside a payload) — recurse.
    return nested === undefined ? value : parseEmbeddedJson(nested);
  }
  if (Array.isArray(value)) {
    return value.map(parseEmbeddedJson);
  }
  if (value !== null && typeof value === "object") {
    const out: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(value)) {
      out[k] = parseEmbeddedJson(v);
    }
    return out;
  }
  return value;
}

/** Parse `s` only when it is a JSON **object or array** literal; otherwise `undefined`. We deliberately
 *  do NOT unwrap a bare `"42"`/`"true"`/`"\"hi\""` — those are faithful strings, and re-typing them
 *  would silently change the data the author sees. */
function tryParseJsonContainer(s: string): unknown {
  const trimmed = s.trim();
  const looksLikeContainer =
    (trimmed.startsWith("{") && trimmed.endsWith("}")) ||
    (trimmed.startsWith("[") && trimmed.endsWith("]"));
  if (!looksLikeContainer) return undefined;
  try {
    const parsed = JSON.parse(trimmed);
    return parsed !== null && typeof parsed === "object" ? parsed : undefined;
  } catch {
    // Not actually JSON (e.g. a message that merely starts with `{`) — leave it as a faithful string.
    return undefined;
  }
}
