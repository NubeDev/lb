// Fill a control's `argsTemplate` with the interaction value (widget-builder scope, open-Q4 lean: a
// typed `argsTemplate` with one `{{value}}` slot). A control declares `action = { tool, argsTemplate }`;
// on interaction we deep-substitute every `"{{value}}"` leaf with the control state (the switch bool,
// the slider number, the button's configured payload) and call the write tool through the bridge.
//
// This now DELEGATES to the shared vars library (widget-config-vars scope): `interpolateArgs` treats
// `{{value}}`/`${__value}` as the runtime interaction value, type-preserving (a number stays a number,
// a bool a bool), so a topic like `"acme/cooler/defrost"` is untouched. One substitution engine, reused.

import { interpolateArgs, emptyScope } from "@/lib/vars";

/** Deep-substitute `{{value}}` leaves in `template` with `value`, preserving the value's type. */
export function fillArgs(
  template: Record<string, unknown> | undefined,
  value: unknown,
): Record<string, unknown> {
  if (!template) return {};
  return interpolateArgs(template, emptyScope(), value) as Record<string, unknown>;
}
