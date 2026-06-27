// Trust-tier routing for a widget renderer (widget-builder scope, "No in-process untrusted code").
// The one rule the platform does NOT bend: arbitrary AUTHOR code in the shell process is RCE. The
// trust class is decided by WHO authored the code, and that splits cleanly:
//
//   - An INSTALLED EXTENSION widget renders IN-PROCESS (module-federates against the shell's React
//     singleton, native-feeling). Installing an extension already requires the publish/install
//     capability — a developer/admin decision to run that code on the node — so the install IS the
//     trust gate. A federated remote externalizes React expecting the shell's import map to resolve
//     it, which only exists in-process; the sandbox can't load it anyway. (See
//     debugging/frontend/ext-widget-iframe-tier-cannot-resolve-bare-react.md.)
//   - A SCRIPTED VIEW (Plot/D3/JSX `template`) renders in a SANDBOXED IFRAME, always. That code is
//     typed by a dashboard EDITOR into a cell — untrusted, never in-process.
//
// So: extension widget ⇒ in-process; scripted author code ⇒ iframe. The `VITE_TRUSTED_WIDGET_KEYS`
// allow-list remains for a future tier that would further restrict WHICH installed publishers may
// federate (default: every installed widget federates — the install is the gate); it is shell
// configuration, never data an extension can influence.

/** The configured allow-list of trusted publisher key ids (shell config; for a future
 *  restrict-which-publisher-federates tier). Empty unless the shell sets the env var. */
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

/** Decide the render tier for an INSTALLED extension widget. An installed extension passed the
 *  publish/install capability gate, so its widget federates IN-PROCESS (the tier its bundle is built
 *  for — bare `react` imports resolve via the shell import map). Scripted author code never reaches
 *  here — it is unconditionally iframe (see {@link scriptedTier}). The `publisherKeyId` is accepted
 *  for a future allow-list-restricted federation tier; today every installed widget federates. */
export function extWidgetTier(_publisherKeyId?: string | undefined | null): TrustTier {
  return "in-process";
}

/** Scripted views (plot/d3/template) are ALWAYS sandboxed — author code never runs in-process. */
export function scriptedTier(): TrustTier {
  return "iframe";
}

/** True if `key` is on the configured allow-list (exposed for the trust-tier routing test). */
export function isTrustedKey(key: string | undefined | null): boolean {
  return !!key && trustedKeys().has(key);
}
