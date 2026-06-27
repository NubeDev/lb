// Trust-tier routing for a widget renderer (widget-builder scope, "No in-process untrusted code").
// The one rule the platform does NOT bend: arbitrary author code in the shell process is RCE. So:
//
//   - An allow-listed publisher KEY → its extension widget may module-federate IN-PROCESS (shares the
//     shell's React singleton, native-feeling), exactly like the trusted page tier.
//   - Everything else — a non-allow-listed extension widget AND every scripted view (Plot/D3/JSX
//     `template`) — renders in a SANDBOXED IFRAME. A non-allow-listed key can NEVER federate
//     in-process even if its manifest asks.
//
// The allow-list is shell configuration (a publisher-key set), NOT data an extension can influence —
// an extension naming itself trusted in its manifest changes nothing here. Default: EMPTY, so an
// unconfigured shell iframes every widget (safe by default; in-process is the opt-in). Configured via
// `VITE_TRUSTED_WIDGET_KEYS` (comma-separated publisher key ids) for the real shell.

/** The configured allow-list of trusted publisher key ids. Empty unless the shell sets the env var. */
function trustedKeys(): Set<string> {
  const raw = (import.meta.env.VITE_TRUSTED_WIDGET_KEYS as string | undefined) ?? "";
  return new Set(
    raw
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean),
  );
}

/** The two render tiers a widget can land in. */
export type TrustTier = "in-process" | "iframe";

/** Decide the render tier for an extension widget whose artifact was signed by `publisherKeyId`.
 *  An allow-listed key federates in-process; anything else iframes. Scripted views never reach here —
 *  they are unconditionally iframe (see {@link scriptedTier}). */
export function extWidgetTier(publisherKeyId: string | undefined | null): TrustTier {
  if (publisherKeyId && trustedKeys().has(publisherKeyId)) return "in-process";
  return "iframe";
}

/** Scripted views (plot/d3/template) are ALWAYS sandboxed — author code never runs in-process. */
export function scriptedTier(): TrustTier {
  return "iframe";
}

/** True if `key` is on the configured allow-list (exposed for the trust-tier routing test). */
export function isTrustedKey(key: string | undefined | null): boolean {
  return !!key && trustedKeys().has(key);
}
