// The `boolean` arg widget (channel rich responses scope) — an `x-lb-widget:"boolean"` arg renders a
// checkbox on the palette's `{ value, onChange }` contract. The value is the STRING `"true"`/`"false"`
// in the chip model (the submit path coerces it to a real bool — e.g. a reminder's `enabled`). RENDER +
// local input only (FILE-LAYOUT).

interface Props {
  name: string;
  value: string;
  onChange: (value: string) => void;
}

export function BooleanArg({ name, value, onChange }: Props) {
  const checked = value === "true";
  return (
    <label className="mx-3 my-2 flex items-center gap-2 text-xs text-fg" aria-label={name}>
      {/* eslint-disable-next-line no-restricted-syntax -- a native checkbox, not a shadcn Input shape */}
      <input
        type="checkbox"
        aria-label={name}
        checked={checked}
        onChange={(e) => onChange(e.target.checked ? "true" : "false")}
        className="h-4 w-4 rounded border-border"
      />
      {name}
    </label>
  );
}
