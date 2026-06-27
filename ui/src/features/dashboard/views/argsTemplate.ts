// Fill a control's `argsTemplate` with the interaction value (widget-builder scope, open-Q4 lean: a
// typed `argsTemplate` with one `{{value}}` slot). A control declares `action = { tool, argsTemplate }`;
// on interaction we deep-substitute every `"{{value}}"` leaf with the control state (the switch bool,
// the slider number, the button's configured payload) and call the write tool through the bridge.
//
// Only the exact string `"{{value}}"` is replaced (typed: the substituted value keeps its real type —
// a number stays a number, a bool a bool), so a topic like `"acme/cooler/defrost"` is untouched.

/** Deep-substitute `{{value}}` leaves in `template` with `value`, preserving the value's type. */
export function fillArgs(
  template: Record<string, unknown> | undefined,
  value: unknown,
): Record<string, unknown> {
  if (!template) return {};
  return subst(template, value) as Record<string, unknown>;
}

function subst(node: unknown, value: unknown): unknown {
  if (node === "{{value}}") return value;
  if (Array.isArray(node)) return node.map((n) => subst(n, value));
  if (node && typeof node === "object") {
    const out: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(node)) out[k] = subst(v, value);
    return out;
  }
  return node;
}
