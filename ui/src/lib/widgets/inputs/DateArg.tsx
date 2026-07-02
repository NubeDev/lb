// The `date` arg widget (channel rich responses scope) — an `x-lb-widget:"date"` arg renders a native
// date input on the palette's `{ value, onChange }` contract (an ISO `YYYY-MM-DD` string). RENDER +
// local input only (FILE-LAYOUT).

import { Input } from "@/components/ui/input";

interface Props {
  name: string;
  value: string;
  onChange: (value: string) => void;
  onSubmit: () => void;
  onCancel: () => void;
}

export function DateArg({ name, value, onChange, onSubmit, onCancel }: Props) {
  return (
    <Input
      type="date"
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
      className="mx-3 my-2 w-[calc(100%-1.5rem)]"
    />
  );
}
