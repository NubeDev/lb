// The arg-side extension widget (channel rich responses scope) — the FORM-side mount for an
// `ext:<id>/<widget>` arg hint. The registry resolves an `ext:` widget to an `ext` entry (OPEN
// vocabulary, never a crash); this renders it. See registry.ts for the DECISION: the shipped
// ext-widget federation mount (`RemoteWidgetMount`/`mountWidget`, the RESPONSE-side path) is a
// bridge-driven, self-owned tile with NO value/onChange callback, so a federated ARG widget cannot
// report a collected form value back to the palette through the shipped contract. Until the mount
// contract gains a value channel, an arg-side ext widget collects its value through a plain text input
// (the same honest fallback an unknown widget takes) — so the palette stays generic and never crashes on
// an ext arg hint. RESPONSE-side ext widgets are unaffected: they mount for real via WidgetView→ExtWidget.
//
// One responsibility: render the arg-side value input for an ext widget (a labelled text field today).

import { TextArg } from "./TextArg";

interface Props {
  /** The active arg name. */
  name: string;
  /** The current value for this arg. */
  value: string;
  onChange: (value: string) => void;
  /** ⏎ — submit the whole command. */
  onSubmit: () => void;
  /** Esc — back out of the arg. */
  onCancel: () => void;
}

export function ExtArg({ name, value, onChange, onSubmit, onCancel }: Props) {
  // Value-collection fallback: a plain text input (the shipped mount contract has no value channel yet).
  return <TextArg name={name} value={value} onChange={onChange} onSubmit={onSubmit} onCancel={onCancel} />;
}
