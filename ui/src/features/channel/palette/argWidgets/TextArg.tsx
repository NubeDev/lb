// The `text` arg widget (channel rich responses scope) — the default/fallback arg input: a plain text
// field on the palette's `{ value, onChange, onSubmit, onCancel }` contract. It is also what the widget
// registry falls back to for an UNKNOWN `x-lb-widget` (never crash). ⏎ submits; Esc cancels. RENDER +
// local input only (FILE-LAYOUT).

import { Input } from "@/components/ui/input";

interface Props {
  name: string;
  value: string;
  onChange: (value: string) => void;
  onSubmit: () => void;
  onCancel: () => void;
}

export function TextArg({ name, value, onChange, onSubmit, onCancel }: Props) {
  return (
    <Input
      aria-label={name}
      value={value}
      autoFocus
      onChange={(e) => onChange(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          onSubmit();
        } else if (e.key === "Escape") {
          e.preventDefault();
          onCancel();
        }
      }}
      placeholder={`${name}…`}
      className="mx-3 my-2 w-[calc(100%-1.5rem)]"
    />
  );
}
