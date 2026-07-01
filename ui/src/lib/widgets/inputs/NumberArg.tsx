// The `number` arg widget (channel rich responses scope) — an `x-lb-widget:"number"` arg renders a
// numeric input on the palette's `{ value, onChange }` contract. The value stays a STRING in the chip
// model (the submit path coerces it — a number arg's `max_runs` becomes a real number there); this
// widget is RENDER + local input only (FILE-LAYOUT).

import { Input } from "@/components/ui/input";

interface Props {
  name: string;
  value: string;
  onChange: (value: string) => void;
  onSubmit: () => void;
  onCancel: () => void;
}

export function NumberArg({ name, value, onChange, onSubmit, onCancel }: Props) {
  return (
    <Input
      type="number"
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
