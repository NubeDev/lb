// The active-arg widget renderer (channel rich responses scope) — resolves the active arg's
// `x-lb.widget` through the {@link resolveWidget} registry and renders the matching arg widget. Extracted
// from CommandPalette so the palette stays a thin controller under the FILE-LAYOUT line budget: the
// palette owns state + keyboard/submit routing; THIS owns the widget switch. The ENTITY picker stays in
// the palette (it needs the shared ↑↓⏎ mention-nav); everything else (sql/runtime/select/cron/boolean/
// number/date/text) resolves here, with an unknown widget degrading to `text` (never crashes).
//
// Two value channels, matching the palette's model: a CHIP widget (text/number/date) commits its value
// to a chip via `{ value, onChange, onSubmit, onCancel }`; an INLINE widget (sql/runtime/select/cron/
// boolean) writes into the palette's per-arg inline map via `onChange`. The palette folds both into the
// args object on submit.

import type { XLbHint } from "@/lib/channel/palette.types";
import {
  SqlArg,
  RuntimeArg,
  SelectArg,
  CronArg,
  BooleanArg,
  NumberArg,
  DateArg,
  TextArg,
  ExtArg,
} from "@/lib/widgets/inputs";
import { resolveWidget } from "@/lib/widgets/registry";

interface Props {
  /** The active arg name. */
  name: string;
  /** Its resolved `x-lb` hint (drives the widget + a `select`'s options/source). */
  hint: XLbHint | undefined;
  /** The current value for this arg (chip-in-progress text OR the inline widget value). */
  value: string;
  onChange: (value: string) => void;
  /** ⏎ — submit the whole command (chip widgets only; inline widgets have their own affordance). */
  onSubmit: () => void;
  /** Esc — back out of the arg. */
  onCancel: () => void;
  /** The chosen SQL source chip (drives the SQL editor's autocomplete). */
  sqlSource: string | null;
}

export function ActiveArgWidget({ name, hint, value, onChange, onSubmit, onCancel, sqlSource }: Props) {
  const { kind } = resolveWidget(hint);
  switch (kind) {
    case "sql":
      return <SqlArg source={sqlSource} value={value} onChange={onChange} onSubmit={onSubmit} onCancel={onCancel} />;
    case "runtime":
      return <RuntimeArg value={value} onChange={onChange} />;
    case "select":
      return <SelectArg value={value} onChange={onChange} options={hint?.options} source={hint?.source} />;
    case "cron":
      return <CronArg value={value} onChange={onChange} />;
    case "boolean":
      return <BooleanArg name={name} value={value} onChange={onChange} />;
    case "number":
      return <NumberArg name={name} value={value} onChange={onChange} onSubmit={onSubmit} onCancel={onCancel} />;
    case "date":
      return <DateArg name={name} value={value} onChange={onChange} onSubmit={onSubmit} onCancel={onCancel} />;
    case "ext":
      // An extension-contributed arg widget (`ext:<id>/<widget>`). Value-collection falls back to text
      // today — see ExtArg / registry.ts for the mount-contract decision.
      return <ExtArg name={name} value={value} onChange={onChange} onSubmit={onSubmit} onCancel={onCancel} />;
    // `text` (the fallback) and any unknown widget → a plain text input.
    default:
      return <TextArg name={name} value={value} onChange={onChange} onSubmit={onSubmit} onCancel={onCancel} />;
  }
}
